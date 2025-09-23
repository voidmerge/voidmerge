//! Javascript execution.

use crate::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
        input: serde_json::Value,
    ) -> BoxFut<'_, Result<serde_json::Value>>;
}

/// Dyn [JsExec] type.
pub type DynJsExec = Arc<dyn JsExec + 'static + Send + Sync>;

/// Default Javascript executor type.
pub struct JsExecDefault;

impl JsExecDefault {
    /// Get the default executor instance.
    pub fn create() -> DynJsExec {
        let out: DynJsExec = Arc::new(JsExecDefault);
        out
    }
}

impl JsExec for JsExecDefault {
    fn exec(
        &self,
        setup: JsSetup,
        input: serde_json::Value,
    ) -> BoxFut<'_, Result<serde_json::Value>> {
        Box::pin(
            async move { JS.get_or_init(Js::new).exec(setup, input).await },
        )
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
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
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

        let out = thread.exec(setup.clone(), input).await;

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

        let count = self
            .threads
            .iter()
            .map(|(_, list)| list.len())
            .sum::<usize>();

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
        match self.get_thread(&setup) {
            Some(thread) => thread,
            None => JsThread::new(permit),
        }
    }

    pub fn put_thread(&mut self, setup: JsSetup, thread: JsThread) {
        self.threads.entry(setup).or_default().push(thread);
    }
}

enum Cmd {
    Kill,
    Exec {
        setup: JsSetup,
        input: serde_json::Value,
        output: tokio::sync::oneshot::Sender<Result<serde_json::Value>>,
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
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let (output, r) = tokio::sync::oneshot::channel();
        self.cmd_send
            .as_ref()
            .unwrap()
            .send(Cmd::Exec {
                setup,
                input,
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
        let handle = tokio::runtime::Handle::current();
        let thread = std::thread::spawn(move || {
            let on_drop = on_drop;

            let mut cur_setup;
            let mut cur_input;
            let mut cur_output;

            match cmd_recv.blocking_recv() {
                None => return,
                Some(Cmd::Kill) => return,
                Some(Cmd::Exec {
                    setup,
                    input,
                    output,
                }) => {
                    cur_setup = setup;
                    cur_input = input;
                    cur_output = output;
                }
            }

            loop {
                let mut rust = rustyscript::Runtime::with_tokio_runtime_handle(
                    rustyscript::RuntimeOptions {
                        timeout: cur_setup.timeout,
                        max_heap_size: Some(cur_setup.heap_size),
                        ..Default::default()
                    },
                    handle.clone(),
                )
                .unwrap();

                rust.register_function("toUtf8", |args| {
                    let s = args[0].as_str().unwrap();
                    Ok(s.as_bytes().to_vec().into())
                })
                .unwrap();

                rust.register_async_function("bob", |args| {
                    Box::pin(async move { Ok(format!("{args:?}").into()) })
                })
                .unwrap();

                rust.eval::<()>(
                    "
                    globalThis.console.log = () => {};
                    globalThis.console.error = () => {};
                    globalThis.TextEncoder = class TextEncoder {
                        encode(s) {
                            return rustyscript.functions.toUtf8(s);
                        }
                    };
                ",
                )
                .unwrap();

                if let Err(err) = rust.eval::<()>(&cur_setup.code) {
                    on_drop.not_ready();
                    let _ = cur_output.send(Err(std::io::Error::other(err)));
                    return;
                }

                loop {
                    let res: Result<serde_json::Value> = match rust
                        .tokio_runtime()
                        .block_on(async {
                            tokio::time::timeout(
                                cur_setup.timeout,
                                rust.call_function_async(
                                    None,
                                    "vm",
                                    rustyscript::json_args!(cur_input),
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
                            input,
                            output,
                        }) => {
                            let reset = cur_setup != setup;
                            cur_setup = setup;
                            cur_input = input;
                            cur_output = output;
                            if reset {
                                println!("reset");
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
