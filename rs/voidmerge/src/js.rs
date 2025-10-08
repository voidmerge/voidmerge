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
    /// Valitate an object to be stored.
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

/// Set the max thread count. (Default: 512).
pub fn js_global_set_max_thread(count: usize) -> bool {
    MAX_THREADS.set(count).is_ok()
}

fn js_global_get_max_thread() -> usize {
    *MAX_THREADS.get_or_init(|| 512)
}

/// Javascript setup info.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JsSetup {
    /// The current context.
    pub ctx: Arc<str>,

    /// Maximum execution time. Default: 10s.
    pub timeout: std::time::Duration,

    /// Max heap size for the context. Default: 32 MiB.
    pub heap_size: usize,

    /// Javascript code to initialize.
    pub code: Arc<str>,
}

impl Default for JsSetup {
    fn default() -> Self {
        Self {
            ctx: Default::default(),
            timeout: std::time::Duration::from_secs(10),
            heap_size: 1024 * 1024 * 32,
            code: Default::default(),
        }
    }
}

static JS: std::sync::OnceLock<Js> = std::sync::OnceLock::new();

/// Javascript executor type.
pub trait JsExec: 'static + Send + Sync {
    /// Execute some javascript code.
    fn exec(
        &self,
        setup: JsSetup,
        obj: crate::obj::ObjWrap,
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
        obj: crate::obj::ObjWrap,
        request: JsRequest,
    ) -> BoxFut<'_, Result<JsResponse>> {
        Box::pin(async move {
            JS.get_or_init(Js::new)
                .exec(setup, obj, request, self.0.clone())
                .await
        })
    }
}

/// Javascript execution.
struct Js {
    limit: Arc<tokio::sync::Semaphore>,
    pool: Arc<Mutex<JsPool>>,
}

impl Js {
    pub fn new() -> Self {
        let max_threads = js_global_get_max_thread();
        Self {
            limit: Arc::new(tokio::sync::Semaphore::new(max_threads)),
            pool: Arc::new(Mutex::new(JsPool::new(max_threads))),
        }
    }

    pub async fn exec(
        &self,
        setup: JsSetup,
        obj: crate::obj::ObjWrap,
        request: JsRequest,
        weak: WeakJsExec,
    ) -> Result<JsResponse> {
        let mut found = self.pool.lock().unwrap().get_thread(&setup);

        if found.is_none() {
            let permit = self
                .limit
                .clone()
                .acquire_owned()
                .await
                .expect("permit error");

            found = Some(
                self.pool
                    .lock()
                    .unwrap()
                    .get_or_create_thread(permit, &setup),
            );
        }

        let thread = found.unwrap();

        let out = thread.exec(setup.clone(), obj, request, weak).await;

        if thread.is_ready() {
            self.pool.lock().unwrap().put_thread(setup, thread);
        }

        out
    }
}

struct JsPool {
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

    pub fn get_thread(&mut self, want_setup: &JsSetup) -> Option<JsThread> {
        if self.last_prune.elapsed() > std::time::Duration::from_secs(5) {
            self.last_prune = std::time::Instant::now();
            self.threads.retain(|_, list| !list.is_empty());
        }

        // first try for a setup match
        if let Some(list) = self.threads.get_mut(want_setup) {
            while !list.is_empty() {
                let thread = list.remove(0);
                if thread.is_ready() {
                    return Some(thread);
                }
            }
        }

        let count = self.threads.values().map(|list| list.len()).sum::<usize>();

        if count < self.max_threads - 2 {
            // go ahead and make a new thread, we've got space
            return None;
        }

        // then, just get any (would be good to introduce some lru here)
        for (_, list) in self.threads.iter_mut() {
            while !list.is_empty() {
                let thread = list.remove(0);
                if thread.is_ready() {
                    return Some(thread);
                }
            }
        }

        None
    }

    pub fn get_or_create_thread(
        &mut self,
        permit: tokio::sync::OwnedSemaphorePermit,
        setup: &JsSetup,
    ) -> JsThread {
        match self.get_thread(setup) {
            Some(thread) => thread,
            None => JsThread::new(permit),
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
    pub obj: crate::obj::ObjWrap,
}

impl TState {
    pub fn new(
        setup: JsSetup,
        weak: WeakJsExec,
        obj: crate::obj::ObjWrap,
    ) -> Self {
        TState { setup, weak, obj }
    }
}

mod deno_ext {
    use super::*;

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

    #[derive(Debug, serde::Deserialize)]
    struct PutMeta {
        #[serde(rename = "appPath", default)]
        app_path: String,

        #[serde(rename = "expiresSecs", default)]
        expires_secs: f64,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_obj_put(
        state: Rc<RefCell<OpState>>,
        #[buffer(copy)] data: bytes::Bytes,
        #[serde] put_meta: PutMeta,
    ) -> std::result::Result<Arc<str>, deno_core::error::CoreError> {
        let (setup, weak, obj) = match state.borrow().try_borrow::<TState>() {
            Some(TState {
                setup, weak, obj, ..
            }) => (setup.clone(), weak.clone(), obj.clone()),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let meta = crate::obj::ObjMeta::new_context(
            &setup.ctx,
            &put_meta.app_path,
            safe_now(),
            put_meta.expires_secs,
        );

        if let Some(exec) = weak.upgrade() {
            match exec
                .exec(
                    setup.clone(),
                    obj.clone(),
                    JsRequest::ObjCheckReq {
                        data: data.clone(),
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

        obj.put(meta.clone(), data).await.map_err(|err| {
            deno_core::error::CoreError::from(
                deno_core::error::CoreErrorKind::Io(err),
            )
        })?;

        Ok(meta.0)
    }

    #[derive(serde::Serialize)]
    struct ObjGetResult {
        meta: crate::obj::ObjMeta,
        data: Bytes,
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_obj_get(
        state: Rc<RefCell<OpState>>,
        #[string] meta: String,
    ) -> std::result::Result<ObjGetResult, deno_core::error::CoreError> {
        let (setup, obj) = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, obj, .. }) => (setup.clone(), obj.clone()),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let meta = crate::obj::ObjMeta(meta.into());
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
        let (meta, data) = obj.get(meta).await.map_err(|err| {
            deno_core::error::CoreError::from(
                deno_core::error::CoreErrorKind::Io(err),
            )
        })?;

        Ok(ObjGetResult { meta, data })
    }

    #[deno_core::op2(async)]
    #[serde]
    async fn op_obj_list(
        state: Rc<RefCell<OpState>>,
        #[string] path_prefix: String,
        created_gt: f64,
        limit: f64,
    ) -> std::result::Result<
        Vec<crate::obj::ObjMeta>,
        deno_core::error::CoreError,
    > {
        let (setup, obj) = match state.borrow().try_borrow::<TState>() {
            Some(TState { setup, obj, .. }) => (setup.clone(), obj.clone()),
            _ => {
                return Err(deno_core::error::CoreErrorKind::Io(Error::other(
                    "bad state",
                ))
                .into());
            }
        };

        let path = format!(
            "{}/{}/{path_prefix}",
            crate::obj::ObjMeta::SYS_CTX,
            setup.ctx
        );

        let limit = limit.clamp(0.0, 1000.0) as u32;

        let result =
            obj.list(&path, created_gt, limit).await.map_err(|err| {
                deno_core::error::CoreError::from(
                    deno_core::error::CoreErrorKind::Io(err),
                )
            })?;

        Ok(result)
    }

    deno_core::extension!(
        vm,
        deps = [deno_console],
        ops = [
            op_to_utf8,
            op_from_utf8,
            op_obj_put,
            op_obj_get,
            op_obj_list,
        ],
        esm_entry_point = "ext:vm/entry.js",
        esm = [ dir "src/js", "entry.js" ],
    );
}

//rustyscript::deno_core::extension!(

#[allow(clippy::large_enum_variant)]
enum Cmd {
    Kill,
    Exec {
        setup: JsSetup,
        obj: crate::obj::ObjWrap,
        request: JsRequest,
        weak: WeakJsExec,
        output: tokio::sync::oneshot::Sender<Result<JsResponse>>,
    },
}

struct JsThread {
    _permit: tokio::sync::OwnedSemaphorePermit,
    is_ready: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    cmd_send: Option<tokio::sync::mpsc::Sender<Cmd>>,
}

impl Drop for JsThread {
    fn drop(&mut self) {
        let cmd_send = self.cmd_send.take();
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
    }
}

impl JsThread {
    pub fn is_ready(&self) -> bool {
        self.is_ready.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn exec(
        &self,
        setup: JsSetup,
        obj: crate::obj::ObjWrap,
        request: JsRequest,
        weak: WeakJsExec,
    ) -> Result<JsResponse> {
        let (output, r) = tokio::sync::oneshot::channel();
        self.cmd_send
            .as_ref()
            .unwrap()
            .send(Cmd::Exec {
                setup,
                obj,
                request,
                weak,
                output,
            })
            .await
            .map_err(|_| std::io::Error::other("thread error"))?;
        r.await.map_err(|_| std::io::Error::other("thread error"))?
    }

    pub fn new(permit: tokio::sync::OwnedSemaphorePermit) -> Self {
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
            let mut cur_obj;
            let mut cur_request;
            let mut cur_weak;
            let mut cur_output;

            match cmd_recv.blocking_recv() {
                None => return,
                Some(Cmd::Kill) => return,
                Some(Cmd::Exec {
                    setup,
                    obj,
                    request,
                    weak,
                    output,
                }) => {
                    cur_setup = setup;
                    cur_obj = obj;
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

                rust.put(TState::new(
                    cur_setup.clone(),
                    cur_weak.clone(),
                    cur_obj,
                ))
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
                            obj,
                            request,
                            weak,
                            output,
                        }) => {
                            let reset = cur_setup != setup;
                            cur_setup = setup;
                            cur_obj = obj;
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
            _permit: permit,
            thread: Some(thread),
            cmd_send: Some(cmd_send),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn js_simple() {
        let obj = obj::obj_file::ObjFile::create(None).await.unwrap();
        let obj = obj::ObjWrap::new(obj).await.unwrap();

        let setup = JsSetup {
            ctx: "bobbo".into(),
            code: "
async function vm(req) {
    if (req.type === 'objCheckReq') {
        return { type: 'objCheckResOk' };
    } else if (req.type === 'fnReq') {
        const b = (new TextEncoder()).encode('hello');
        console.log('encode', b, b instanceof Uint8Array);
        const s = (new TextDecoder()).decode(b);
        console.log('decode', s);

        const t = Date.now() / 1000.0;

        const meta = await objPut(
            (new TextEncoder()).encode('hello'),
            {
                appPath: 'test',
                createdSecs: t,
            }
        );
        console.log(`put returned meta: ${meta}`);

        const res = (new TextDecoder()).decode((await objGet(meta)).data);
        console.log(`fetched: ${res}`);

        let count = (await objList('t', 0.0, 42)).length;

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
            ..Default::default()
        };

        let req = JsRequest::FnReq {
            method: "GET".into(),
            path: "foo/bar".into(),
            body: None,
            headers: Default::default(),
        };

        let js = JsExecDefault::create();

        let res = js
            .exec(setup.clone(), obj.clone(), req.clone())
            .await
            .unwrap();
        println!("got: {res:#?}");
        let res = js.exec(setup, obj.clone(), req).await.unwrap();
        println!("got: {res:#?}");

        let prefix = format!("{}/bobbo/", crate::obj::ObjMeta::SYS_CTX);
        let p = obj.list(&prefix, 0.0, u32::MAX).await.unwrap();
        for meta in p {
            println!("GOT: {meta:?}");
        }
    }
}
