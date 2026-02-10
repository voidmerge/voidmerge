//! Javascript execution.

use crate::*;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Input to a javascript execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JsRequest {
    /// Get the code config.
    CodeConfigReq,
    /// Execute the cron code.
    CronReq,
    /// Validate an object to be stored.
    ObjCheckReq {
        /// The content payload of the object.
        data: Bytes,

        /// The metadata of the object.
        meta: crate::obj::ObjMeta,
    },
    /// Incoming function request.
    FnReq {
        /// The method ("GET" or "PUT").
        method: String,
        /// The request url.
        path: String,
        /// The body content.
        body: Option<Bytes>,
        /// Any sent headers.
        headers: HashMap<String, String>,
    },
}

fn status() -> f64 {
    200.0
}

/// Output from a javascript execution.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JsResponse {
    /// Return this for code config requests.
    CodeConfigResOk {
        /// Interval for running cron executions.
        #[serde(default)]
        cron_interval_secs: Option<f64>,
    },

    /// Cron Ok Response.
    CronResOk,

    /// Return this in case of ObjCheck request success.
    ObjCheckResOk,

    /// Outgoing function response.
    FnResOk {
        /// The status code to respond with.
        #[serde(default = "status")]
        status: f64,
        /// The body content.
        #[serde(default)]
        body: Bytes,
        /// Any headers to send.
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

static MAX_THREADS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Set the max thread count. (Default: 32).
pub fn js_global_set_max_thread(count: usize) -> bool {
    MAX_THREADS.set(count).is_ok()
}

fn js_global_get_max_thread() -> usize {
    *MAX_THREADS.get_or_init(|| 32)
}

static MAX_RAM: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Set max RAM to use. (Default: 768 MiB).
pub fn js_global_set_max_ram(count: usize) -> bool {
    MAX_RAM.set(count).is_ok()
}

fn js_global_get_max_ram() -> usize {
    *MAX_RAM.get_or_init(|| 768 * 1024 * 1024)
}

/// Javascript setup info.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JsSetup {
    /// The current VoidMerge runtime.
    pub runtime: Runtime,

    /// The current context.
    pub ctx: Arc<str>,

    /// Maximum execution time. Default: 10s.
    pub timeout: std::time::Duration,

    /// Max heap size for the context. Default: 32 MiB.
    pub heap_size: usize,

    /// Javascript code to initialize.
    pub code: Arc<str>,

    /// Javascript env to make available.
    pub env: Arc<serde_json::Value>,
}

impl JsSetup {
    /// Default timeout.
    pub const DEF_TIMEOUT: std::time::Duration =
        std::time::Duration::from_secs(10);

    /// Default heap size.
    pub const DEF_HEAP_SIZE: usize = 1024 * 1024 * 32;
}

static JS: std::sync::OnceLock<Js> = std::sync::OnceLock::new();

/// Javascript executor type.
pub trait JsExec: 'static + Send + Sync {
    /// Execute some javascript code.
    fn exec(
        &self,
        setup: JsSetup,
        request: JsRequest,
    ) -> BoxFut<'_, Result<JsResponse>>;
}

/// Dyn [JsExec] type.
pub type DynJsExec = Arc<dyn JsExec + 'static + Send + Sync>;
type WeakJsExec = std::sync::Weak<dyn JsExec + 'static + Send + Sync>;

/// Default Javascript executor type.
pub struct JsExecDefault(WeakJsExec);

impl JsExecDefault {
    /// Get the default executor instance.
    pub fn create() -> DynJsExec {
        let out: DynJsExec = Arc::new_cyclic(|this: &std::sync::Weak<Self>| {
            JsExecDefault(this.clone())
        });
        out
    }
}

impl JsExec for JsExecDefault {
    fn exec(
        &self,
        setup: JsSetup,
        request: JsRequest,
    ) -> BoxFut<'_, Result<JsResponse>> {
        Box::pin(async move {
            JS.get_or_init(Js::new)
                .exec(setup, request, self.0.clone())
                .await
        })
    }
}

/// Javascript Executor Wrapper Adding Metering.
pub struct JsExecMeter(pub DynJsExec);

impl JsExecMeter {
    /// Create a JsExecMeter wrapper around another javascript executor.
    pub fn create(inner: DynJsExec) -> DynJsExec {
        let out: DynJsExec = Arc::new(Self(inner));
        out
    }
}

impl JsExec for JsExecMeter {
    fn exec(
        &self,
        setup: JsSetup,
        request: JsRequest,
    ) -> BoxFut<'_, Result<JsResponse>> {
        Box::pin(async move {
            let ctx = setup.ctx.clone();
            let mem = setup.heap_size;

            let start = std::time::Instant::now();
            let res = self.0.exec(setup, request).await;
            let mut elapsed_millis = start.elapsed().as_millis();

            if elapsed_millis < 100 {
                elapsed_millis = 100;
            }

            crate::meter::meter_fn_mib_milli(
                &ctx,
                (mem as u128 * elapsed_millis) / 1048576,
            );

            res
        })
    }
}

/// Javascript execution.
struct Js {
    thread_limit: Arc<tokio::sync::Semaphore>,
    ram_mib_limit: Arc<tokio::sync::Semaphore>,
    pool: Arc<Mutex<JsPool>>,
}

impl Js {
    pub fn new() -> Self {
        let max_threads = js_global_get_max_thread();
        let max_ram = js_global_get_max_ram();
        if max_ram < 1024 * 1024 {
            panic!("max ram cannot be less that 1MiB");
        }
        let max_ram_mib = max_ram / (1024 * 1024);
        if max_ram_mib > u32::MAX as usize {
            panic!("max ram is too large in MiB for a u32");
        }
        Self {
            thread_limit: Arc::new(tokio::sync::Semaphore::new(max_threads)),
            ram_mib_limit: Arc::new(tokio::sync::Semaphore::new(max_ram_mib)),
            pool: Arc::new(Mutex::new(JsPool::new(max_threads))),
        }
    }

    pub async fn exec(
        &self,
        setup: JsSetup,
        request: JsRequest,
        weak: WeakJsExec,
    ) -> Result<JsResponse> {
        let avail = self.ram_mib_limit.available_permits() * 1024 * 1024;
        let want = setup.heap_size;
        let clear = want.saturating_sub(avail);
        let mut found = self.pool.lock().unwrap().get_thread(&setup, clear);

        if found.is_none() {
            let t_fut = self.thread_limit.clone().acquire_owned();

            if setup.heap_size < 1024 * 1024 {
                panic!("heap_size cannot be less than 1 MiB");
            }

            let r_fut = self
                .ram_mib_limit
                .clone()
                .acquire_many_owned((setup.heap_size / (1024 * 1024)) as u32);

            let (thread_permit, ram_permit) =
                tokio::try_join!(t_fut, r_fut).expect("permit error");

            found = Some(self.pool.lock().unwrap().get_or_create_thread(
                thread_permit,
                ram_permit,
                &setup,
            ));
        }

        let thread = found.unwrap();

        let out = thread.exec(setup.clone(), request, weak).await;

        // if the thread errored, don't return it
        // if we are out of permits, don't return it
        if thread.is_ready() && self.ram_mib_limit.available_permits() > 0 {
            self.pool.lock().unwrap().put_thread(setup, thread);
        }

        out
    }
}

struct JsPool {
    #[allow(dead_code)]
    max_threads: usize,
    last_prune: std::time::Instant,
    threads: HashMap<JsSetup, Vec<JsThread>>,
}

impl JsPool {
    pub fn new(max_threads: usize) -> Self {
        Self {
            max_threads,
            last_prune: std::time::Instant::now(),
            threads: Default::default(),
        }
    }

    pub fn get_thread(
        &mut self,
        want_setup: &JsSetup,
        clear_heap: usize,
    ) -> Option<JsThread> {
        if self.last_prune.elapsed() > std::time::Duration::from_secs(5) {
            self.last_prune = std::time::Instant::now();
            self.threads.retain(|_, list| !list.is_empty());
        }

        // if we have a matching thread cached, return it
        if let Some(list) = self.threads.get_mut(want_setup) {
            while !list.is_empty() {
                let thread = list.remove(0);
                if thread.is_ready() {
                    return Some(thread);
                }
            }
        }

        // otherwise, try to clear enough space for the request
        let mut clear_amount = 0;
        self.threads.retain(|setup, list| {
            list.retain(|_| {
                if clear_amount < clear_heap {
                    clear_amount += setup.heap_size;
                    false
                } else {
                    true
                }
            });
            !list.is_empty()
        });

        None
    }

    pub fn get_or_create_thread(
        &mut self,
        thread_permit: tokio::sync::OwnedSemaphorePermit,
        ram_permit: tokio::sync::OwnedSemaphorePermit,
        setup: &JsSetup,
    ) -> JsThread {
        // we can set a clear heap size of zero here,
        // since we already got the permit.
        match self.get_thread(setup, 0) {
            Some(thread) => thread,
            None => JsThread::new(thread_permit, ram_permit),
        }
    }

    pub fn put_thread(&mut self, setup: JsSetup, thread: JsThread) {
        self.threads.entry(setup).or_default().push(thread);
    }
}

use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;

struct TState {
    pub setup: JsSetup,
    pub weak: WeakJsExec,
}

impl TState {
    pub fn new(setup: JsSetup, weak: WeakJsExec) -> Self {
        TState { setup, weak }
    }
}

mod deno_ext {
    use super::*;

    #[deno_core::op2]
    #[serde]
    fn op_get_ctx(
        state: Rc<RefCell<OpState>>,
    ) -> std::result::Result<Arc<str>, deno_core::error::CoreError> {
        match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => Ok(setup.ctx.clone()),
            _ => Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "bad state",
            ))
            .into()),
        }
    }

    #[deno_core::op2]
    #[serde]
    fn op_get_env(
        state: Rc<RefCell<OpState>>,
    ) -> std::result::Result<Arc<serde_json::Value>, deno_core::error::CoreError>
    {
        match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => Ok(setup.env.clone()),
            _ => Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "bad state",
            ))
            .into()),
        }
    }

    #[deno_core::op2]
    #[buffer]
    fn op_to_utf8(#[string] input: &str) -> Vec<u8> {
        input.as_bytes().to_vec()
    }

    #[deno_core::op2]
    #[string]
    fn op_from_utf8(#[buffer] input: &[u8]) -> String {
        String::from_utf8_lossy(input).to_string()
    }

    #[derive(Debug, serde::Serialize)]
    struct MsgNewOutput {
        #[serde(rename = "msgId")]
        msg_id: Arc<str>,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_msg_new(
        state: Rc<RefCell<OpState>>,
    ) -> std::result::Result<MsgNewOutput, deno_core::error::CoreError> {
        let setup = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => setup.clone(),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let msg_id = setup.runtime.msg()?.create(setup.ctx).await?;

        Ok(MsgNewOutput { msg_id })
    }

    #[derive(Debug, serde::Serialize)]
    struct MsgListOutput {
        #[serde(rename = "msgIdList")]
        msg_id_list: Vec<Arc<str>>,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_msg_list(
        state: Rc<RefCell<OpState>>,
    ) -> std::result::Result<MsgListOutput, deno_core::error::CoreError> {
        let setup = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => setup.clone(),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let msg_id_list = setup.runtime.msg()?.list(setup.ctx).await?;

        Ok(MsgListOutput { msg_id_list })
    }

    #[derive(Debug, serde::Deserialize)]
    struct MsgSendInput {
        #[serde(rename = "msgId")]
        msg_id: Arc<str>,

        msg: bytes::Bytes,
    }

    #[deno_core::op2(async)]
    async fn op_msg_send(
        state: Rc<RefCell<OpState>>,
        #[serde] input: MsgSendInput,
    ) -> std::result::Result<(), deno_core::error::CoreError> {
        let setup = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => setup.clone(),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        setup
            .runtime
            .msg()?
            .send(
                setup.ctx,
                input.msg_id,
                crate::msg::Message::App { msg: input.msg },
            )
            .await?;

        Ok(())
    }

    #[derive(Debug, serde::Deserialize)]
    struct ObjPutInput {
        #[serde(default)]
        meta: Arc<str>,

        #[serde(default)]
        data: bytes::Bytes,
    }

    #[derive(Debug, serde::Serialize)]
    struct ObjPutOutput {
        meta: Arc<str>,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_obj_put(
        state: Rc<RefCell<OpState>>,
        #[serde] input: ObjPutInput,
    ) -> std::result::Result<ObjPutOutput, deno_core::error::CoreError> {
        let (setup, weak) = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, weak }) => (setup.clone(), weak.clone()),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let input_meta = crate::obj::ObjMeta(input.meta);

        let meta = crate::obj::ObjMeta::new_context(
            &setup.ctx,
            input_meta.app_path(),
            safe_now(),
            input_meta.expires_secs(),
            input.data.len() as f64,
        );

        if let Some(exec) = weak.upgrade() {
            match exec
                .exec(
                    setup.clone(),
                    JsRequest::ObjCheckReq {
                        data: input.data.clone(),
                        meta: meta.clone(),
                    },
                )
                .await
            {
                Ok(JsResponse::ObjCheckResOk) => (),
                oth => {
                    return Err(deno_core::error::CoreErrorKind::Io(
                        Error::other(format!(
                            "invalid obj check response: {oth:?}"
                        )),
                    )
                    .into());
                }
            }
        } else {
            return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "aborting obj put due to shutdown",
            ))
            .into());
        }

        setup
            .runtime
            .obj()?
            .put(meta.clone(), input.data)
            .await
            .map_err(|err| {
                deno_core::error::CoreError::from(
                    deno_core::error::CoreErrorKind::Io(err),
                )
            })?;

        Ok(ObjPutOutput { meta: meta.0 })
    }

    #[derive(Debug, serde::Deserialize)]
    struct ObjGetInput {
        #[serde(default)]
        meta: Arc<str>,
    }

    #[derive(Debug, serde::Serialize)]
    struct ObjGetOutput {
        meta: Arc<str>,
        data: Bytes,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_obj_get(
        state: Rc<RefCell<OpState>>,
        #[serde] input: ObjGetInput,
    ) -> std::result::Result<ObjGetOutput, deno_core::error::CoreError> {
        let setup = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => setup.clone(),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let meta = crate::obj::ObjMeta(input.meta);
        if meta.sys_prefix() != crate::obj::ObjMeta::SYS_CTX {
            return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "invalid sys prefix",
            ))
            .into());
        }
        if meta.ctx() != &*setup.ctx {
            return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "invalid sys context",
            ))
            .into());
        }
        let (meta, data) =
            setup.runtime.obj()?.get(meta).await.map_err(|err| {
                deno_core::error::CoreError::from(
                    deno_core::error::CoreErrorKind::Io(err),
                )
            })?;

        Ok(ObjGetOutput { meta: meta.0, data })
    }

    #[derive(Debug, serde::Deserialize)]
    struct ObjRmInput {
        #[serde(default)]
        meta: Arc<str>,
    }

    #[deno_core::op2(async)]
    async fn op_obj_rm(
        state: Rc<RefCell<OpState>>,
        #[serde] input: ObjRmInput,
    ) -> std::result::Result<(), deno_core::error::CoreError> {
        let setup = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => setup.clone(),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let meta = crate::obj::ObjMeta(input.meta);
        if meta.sys_prefix() != crate::obj::ObjMeta::SYS_CTX {
            return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "invalid sys prefix",
            ))
            .into());
        }
        if meta.ctx() != &*setup.ctx {
            return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                "invalid sys context",
            ))
            .into());
        }
        setup.runtime.obj()?.rm(meta).await.map_err(|err| {
            deno_core::error::CoreError::from(
                deno_core::error::CoreErrorKind::Io(err),
            )
        })?;

        Ok(())
    }

    fn f64_1000() -> f64 {
        1000.0
    }

    #[derive(Debug, serde::Deserialize)]
    struct ObjListInput {
        #[serde(rename = "appPathPrefix", default)]
        app_path_prefix: Arc<str>,

        #[serde(rename = "createdGt", default)]
        created_gt: f64,

        #[serde(default = "f64_1000")]
        limit: f64,
    }

    #[derive(Debug, serde::Serialize)]
    struct ObjListOutput {
        #[serde(rename = "metaList")]
        meta_list: Vec<crate::obj::ObjMeta>,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_obj_list(
        state: Rc<RefCell<OpState>>,
        #[serde] input: ObjListInput,
    ) -> std::result::Result<ObjListOutput, deno_core::error::CoreError> {
        let setup = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, .. }) => setup.clone(),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let path = format!(
            "{}/{}/{}",
            crate::obj::ObjMeta::SYS_CTX,
            setup.ctx,
            input.app_path_prefix,
        );

        let limit = input.limit.clamp(0.0, 1000.0) as u32;

        let result = setup
            .runtime
            .obj()?
            .list(&path, input.created_gt, limit)
            .await
            .map_err(|err| {
                deno_core::error::CoreError::from(
                    deno_core::error::CoreErrorKind::Io(err),
                )
            })?;

        Ok(ObjListOutput { meta_list: result })
    }

    deno_core::extension!(
        vm,
        deps = [deno_console],
        ops = [
            op_get_ctx,
            op_get_env,
            op_to_utf8,
            op_from_utf8,
            op_msg_new,
            op_msg_list,
            op_msg_send,
            op_obj_put,
            op_obj_get,
            op_obj_rm,
            op_obj_list,
        ],
        esm_entry_point = "ext:vm/entry.js",
        esm = [ dir "src/js", "entry.js" ],
    );
}

#[allow(clippy::large_enum_variant)]
enum Cmd {
    Kill,
    Exec {
        setup: JsSetup,
        request: JsRequest,
        weak: WeakJsExec,
        output: tokio::sync::oneshot::Sender<Result<JsResponse>>,
    },
}

struct JsThread {
    _thread_permit: tokio::sync::OwnedSemaphorePermit,
    _ram_permit: tokio::sync::OwnedSemaphorePermit,
    is_ready: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    cmd_send: Option<tokio::sync::mpsc::Sender<Cmd>>,
}

impl Drop for JsThread {
    fn drop(&mut self) {
        let cmd_send = self.cmd_send.take();
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::spawn(async move {
                if let Some(cmd_send) = cmd_send {
                    let _ = cmd_send.send(Cmd::Kill).await;
                }
            });
            if let Some(thread) = self.thread.take() {
                tokio::task::spawn_blocking(move || {
                    let _ = thread.join();
                });
            }
        } else {
            let mut dangle = false;
            if let Some(cmd_send) = cmd_send
                && cmd_send.try_send(Cmd::Kill).is_err()
            {
                eprintln!(
                    "FAILED TO SEND KILL, maybe leaving a thread dangling"
                );
                tracing::error!(
                    "FAILED TO SEND KILL, maybe leaving a thread dangling"
                );
                dangle = true;
            }
            if let Some(thread) = self.thread.take()
                && !dangle
            {
                let _ = thread.join();
            }
        }
    }
}

impl JsThread {
    pub fn is_ready(&self) -> bool {
        self.is_ready.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn exec(
        &self,
        setup: JsSetup,
        request: JsRequest,
        weak: WeakJsExec,
    ) -> Result<JsResponse> {
        let (output, r) = tokio::sync::oneshot::channel();
        self.cmd_send
            .as_ref()
            .unwrap()
            .send(Cmd::Exec {
                setup,
                request,
                weak,
                output,
            })
            .await
            .map_err(|_| std::io::Error::other("thread error"))?;
        r.await.map_err(|_| std::io::Error::other("thread error"))?
    }

    pub fn new(
        thread_permit: tokio::sync::OwnedSemaphorePermit,
        ram_permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Self {
        let is_ready = Arc::new(std::sync::atomic::AtomicBool::new(true));

        struct D(Arc<std::sync::atomic::AtomicBool>);

        impl Drop for D {
            fn drop(&mut self) {
                self.not_ready();
            }
        }

        impl D {
            pub fn not_ready(&self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let on_drop = D(is_ready.clone());

        let (cmd_send, mut cmd_recv) = tokio::sync::mpsc::channel(32);
        let thread = std::thread::spawn(move || {
            let on_drop = on_drop;

            let mut cur_setup;
            let mut cur_request;
            let mut cur_weak;
            let mut cur_output;

            match cmd_recv.blocking_recv() {
                None => return,
                Some(Cmd::Kill) => return,
                Some(Cmd::Exec {
                    setup,
                    request,
                    weak,
                    output,
                }) => {
                    cur_setup = setup;
                    cur_request = request;
                    cur_weak = weak;
                    cur_output = output;
                }
            }

            loop {
                let extensions = vec![deno_ext::vm::init()];

                let opts = rustyscript::RuntimeOptions {
                    extensions,
                    timeout: cur_setup.timeout,
                    max_heap_size: Some(cur_setup.heap_size),
                    ..Default::default()
                };

                let mut rust = rustyscript::Runtime::new(opts).unwrap();

                rust.put(TState::new(cur_setup.clone(), cur_weak.clone()))
                    .unwrap();

                if let Err(err) = rust.eval::<()>(&cur_setup.code) {
                    on_drop.not_ready();
                    let _ = cur_output.send(Err(std::io::Error::other(err)));
                    return;
                }

                loop {
                    let res: Result<JsResponse> = match rust
                        .tokio_runtime()
                        .block_on(async {
                            tokio::time::timeout(
                                cur_setup.timeout,
                                rust.call_function_async(
                                    None,
                                    "vm",
                                    rustyscript::json_args!(cur_request),
                                ),
                            )
                            .await
                        }) {
                        Ok(Ok(r)) => Ok(r),
                        Ok(Err(err @ rustyscript::Error::JsError(_))) => {
                            Err(std::io::Error::other(err))
                        }
                        Ok(Err(err)) => {
                            let err = if matches!(
                                err,
                                rustyscript::Error::Runtime(_)
                                    | rustyscript::Error::HeapExhausted
                            ) {
                                std::io::Error::other(format!(
                                    "MemoryError({err:?})"
                                ))
                            } else {
                                std::io::Error::other(err)
                            };
                            on_drop.not_ready();
                            let _ = cur_output.send(Err(err));
                            return;
                        }
                        Err(_) => {
                            on_drop.not_ready();
                            let _ = cur_output
                                .send(Err(std::io::Error::other("Timeout")));
                            return;
                        }
                    };
                    let _ = cur_output.send(res);

                    match cmd_recv.blocking_recv() {
                        None => return,
                        Some(Cmd::Kill) => return,
                        Some(Cmd::Exec {
                            setup,
                            request,
                            weak,
                            output,
                        }) => {
                            let reset = cur_setup != setup;
                            cur_setup = setup;
                            cur_request = request;
                            cur_weak = weak;
                            cur_output = output;
                            if reset {
                                break;
                            }
                        }
                    };
                }
            }
        });
        Self {
            is_ready,
            _thread_permit: thread_permit,
            _ram_permit: ram_permit,
            thread: Some(thread),
            cmd_send: Some(cmd_send),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[ignore = "Run this test in isolation via `cargo test -- --ignored js_stress`"]
    #[tokio::test(flavor = "multi_thread")]
    async fn js_stress() {
        let rth = RuntimeHandle::default();
        let obj = obj::obj_file::ObjFile::create(None).await.unwrap();
        rth.set_obj(obj);

        fn setup(id: usize, runtime: Runtime) -> JsSetup {
            JsSetup {
                runtime,
                ctx: format!("ctx-{id}").into(),
                env: Arc::new(serde_json::Value::Null),
                code: format!(
                    "
async function vm(req) {{
    if (req.type === 'fnReq') {{
        const body = (new TextEncoder()).encode('{id}')
        return {{ type: 'fnResOk', body }};
    }}
    throw new Error('unhandled');
}}
"
                )
                .into(),
                timeout: JsSetup::DEF_TIMEOUT,
                heap_size: JsSetup::DEF_HEAP_SIZE * 5,
            }
        }

        const COUNT: usize = 64;

        let mut setups = Vec::with_capacity(COUNT);
        for id in 0..COUNT {
            setups.push(setup(id, rth.runtime()));
        }

        let js = JsExecDefault::create();

        let req = JsRequest::FnReq {
            method: "GET".into(),
            path: "".into(),
            body: None,
            headers: Default::default(),
        };

        for r in 1..=10 {
            println!("round {r}/10");
            let mut all = Vec::with_capacity(COUNT);
            for id in 0..COUNT {
                all.push(js.exec(setups[id].clone(), req.clone()));
            }
            let res = futures::future::try_join_all(all).await.unwrap();
            assert_eq!(COUNT, res.len());
            for id in 0..COUNT {
                match &res[id] {
                    JsResponse::FnResOk { body, .. } => {
                        let body = String::from_utf8_lossy(body);
                        assert_eq!(id.to_string(), body);
                    }
                    oth => panic!("unexpected result: {oth:?}"),
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn js_simple() {
        let rth = RuntimeHandle::default();
        let obj = obj::obj_file::ObjFile::create(None).await.unwrap();
        rth.set_obj(obj);

        let setup = JsSetup {
            runtime: rth.runtime(),
            ctx: "bobbo".into(),
            env: Arc::new(serde_json::Value::Null),
            code: "
async function vm(req) {
    if (req.type === 'objCheckReq') {
        return { type: 'objCheckResOk' };
    } else if (req.type === 'fnReq') {
        const b = (new TextEncoder()).encode('hello');
        console.log('encode', b, b instanceof Uint8Array);
        const s = (new TextDecoder()).decode(b);
        console.log('decode', s);

        const { meta } = await VM.objPut({
            meta: 'c/A/test',
            data: new TextEncoder().encode('hello'),
        });
        console.log(`put returned meta: ${meta}`);

        const { data } = await VM.objGet({ meta });
        const res = new TextDecoder().decode(data);
        console.log(`fetched: ${res}`);

        const { metaList } = await VM.objList({
            appPathPrefix: 't',
            createdGt: 0.0,
            limit: 42,
        });
        console.log(`list result: ${JSON.stringify(metaList)}`);
        let count = metaList.length;

        if (count !== 1) {
            throw new Error(`failed to list the item`);
        }

        if (res !== 'hello') {
            throw new Error(`bad response, expected 'hello', got: ${res}`);
        }

        return { type: 'fnResOk' };
    } else {
        throw new Error(`invalid type: ${req.type}`);
    }
}
"
            .into(),
            timeout: JsSetup::DEF_TIMEOUT,
            heap_size: JsSetup::DEF_HEAP_SIZE,
        };

        let req = JsRequest::FnReq {
            method: "GET".into(),
            path: "foo/bar".into(),
            body: None,
            headers: Default::default(),
        };

        let js = JsExecDefault::create();

        let res = js.exec(setup.clone(), req.clone()).await.unwrap();
        println!("got: {res:#?}");
        let res = js.exec(setup, req).await.unwrap();
        println!("got: {res:#?}");

        let prefix = format!("{}/bobbo/", crate::obj::ObjMeta::SYS_CTX);
        let p = rth
            .runtime()
            .obj()
            .unwrap()
            .list(&prefix, 0.0, u32::MAX)
            .await
            .unwrap();
        for meta in p {
            println!("GOT: {meta:?}");
        }
    }
}
