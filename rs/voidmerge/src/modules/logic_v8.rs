//! v8 javascript based logic types

use crate::*;
use std::sync::atomic::{AtomicBool, Ordering};
use types::*;

const MAX_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// A [ModuleLogic] module based on the v8 javascript engine.
#[derive(Debug)]
pub struct LogicFactoryV8;

impl ModuleLogicFactory for LogicFactoryV8 {
    fn factory(
        &self,
        _config: Arc<config::Config>,
    ) -> BoxFut<'static, Result<DynModuleLogic>> {
        Box::pin(async move {
            let out: DynModuleLogic = Arc::new(LogicV8(V8ThreadPool::get()));
            Ok(out)
        })
    }
}

const DEFAULT_LOGIC: &str = r#"
VM({
  call: 'register',
  code(i) {
    VM({ call: 'system', type: 'trace', data: i });
    return 'unimplemented';
  }
});
"#;

struct LogicV8(Arc<V8ThreadPool>);

impl ModuleLogic for LogicV8 {
    fn default_logic(&self) -> VmLogic {
        VmLogic::Utf8Single {
            code: DEFAULT_LOGIC.into(),
        }
    }

    fn exec(&self, exec: ModuleLogicExec) -> BoxFut<'_, Result<Value>> {
        let timeout =
            std::cmp::min(MAX_TIMEOUT, exec.timeout.unwrap_or(MAX_TIMEOUT));

        let fut = self.0.exec(exec, timeout);

        Box::pin(fut)
    }
}

type ThreadPair = (V8Thread, tokio::sync::OwnedSemaphorePermit);
struct V8ThreadPool {
    limit: Arc<tokio::sync::Semaphore>,
    thread_send: tokio::sync::mpsc::Sender<ThreadPair>,
    thread_recv: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<ThreadPair>>,
}

impl V8ThreadPool {
    pub fn get() -> Arc<Self> {
        use std::sync::{OnceLock, Weak};

        static SINGLE: OnceLock<Mutex<Weak<V8ThreadPool>>> = OnceLock::new();

        let mut lock = SINGLE.get_or_init(Default::default).lock().unwrap();

        if let Some(this) = lock.upgrade() {
            this
        } else {
            let this = Self::new();
            *lock = Arc::downgrade(&this);
            this
        }
    }

    fn new() -> Arc<Self> {
        let count = num_cpus::get();
        let limit = Arc::new(tokio::sync::Semaphore::new(count));
        let (thread_send, thread_recv) = tokio::sync::mpsc::channel(count);
        let thread_recv = tokio::sync::Mutex::new(thread_recv);
        Arc::new(Self {
            limit,
            thread_send,
            thread_recv,
        })
    }

    async fn exec(
        &self,
        exec: ModuleLogicExec,
        timeout: std::time::Duration,
    ) -> Result<Value> {
        loop {
            let (thread, permit) = if let Ok(permit) =
                self.limit.clone().try_acquire_owned()
            {
                (V8Thread::new().await?, permit)
            } else {
                match tokio::time::timeout(timeout, async {
                    self.thread_recv.lock().await.recv().await
                })
                .await
                {
                    Err(_) => return Err(std::io::ErrorKind::TimedOut.into()),
                    Ok(None) => {
                        return Err(std::io::ErrorKind::BrokenPipe.into());
                    }
                    Ok(Some(r)) => r,
                }
            };

            if thread.should_term() {
                drop(thread);
                drop(permit);
                continue;
            }

            let res = thread.exec(exec, timeout).await;

            if !thread.should_term()
                && self.thread_send.send((thread, permit)).await.is_err()
            {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }

            return res;
        }
    }
}

struct V8Thread {
    should_term: Arc<AtomicBool>,
    term_handle: v8::IsolateHandle,
    exec_send: ExecSendWeak,
    drop: Option<(ExecSend, std::thread::JoinHandle<()>)>,
}

impl Drop for V8Thread {
    fn drop(&mut self) {
        self.terminate();
        if let Some((exec_send, thread)) = self.drop.take() {
            drop(exec_send);
            let _ = thread.join();
        }
    }
}

impl V8Thread {
    async fn new() -> Result<Self> {
        let should_term = Arc::new(AtomicBool::new(false));

        let (term_handle_send, term_handle_recv) =
            tokio::sync::oneshot::channel();

        let (exec_send, exec_recv) = tokio::sync::mpsc::channel(1);

        let should_term2 = should_term.clone();
        let thread = std::thread::spawn(move || {
            v8_thread(should_term2, term_handle_send, exec_recv)
        });

        let term_handle = match term_handle_recv.await {
            Ok(term_handle) => term_handle,
            Err(_) => {
                return Err(std::io::Error::other(
                    "failed to establish v8 thread awaiting term handle",
                ));
            }
        };

        Ok(Self {
            should_term,
            term_handle,
            exec_send: exec_send.downgrade(),
            drop: Some((exec_send, thread)),
        })
    }

    fn terminate(&self) {
        self.should_term.store(true, Ordering::SeqCst);
        self.term_handle.terminate_execution();
    }

    fn should_term(&self) -> bool {
        self.should_term.load(Ordering::SeqCst)
    }

    async fn exec(
        &self,
        exec: ModuleLogicExec,
        timeout: std::time::Duration,
    ) -> Result<Value> {
        let exec_send = match self.exec_send.upgrade() {
            Some(exec_send) => exec_send,
            None => return Err(std::io::ErrorKind::BrokenPipe.into()),
        };

        let (output_send, output_recv) = tokio::sync::oneshot::channel();

        if exec_send
            .send_timeout(Exec { exec, output_send }, timeout)
            .await
            .is_err()
        {
            return Err(std::io::ErrorKind::TimedOut.into());
        }

        match tokio::time::timeout(timeout, output_recv).await {
            Err(_) => Err(std::io::ErrorKind::TimedOut.into()),
            Ok(Err(_)) => Err(std::io::ErrorKind::BrokenPipe.into()),
            Ok(Ok(r)) => r,
        }
    }
}

struct Exec {
    pub exec: ModuleLogicExec,
    pub output_send: tokio::sync::oneshot::Sender<Result<Value>>,
}

type ExecSendWeak = tokio::sync::mpsc::WeakSender<Exec>;
type ExecSend = tokio::sync::mpsc::Sender<Exec>;
type ExecRecv = tokio::sync::mpsc::Receiver<Exec>;

fn fn_cb_logic(
    scope: &mut ::v8::HandleScope,
    args: ::v8::FunctionCallbackArguments,
    mut retval: ::v8::ReturnValue,
) {
    if args.data().is_external() {
        let ext = v8::Local::<v8::External>::try_from(args.data()).unwrap();
        let cb_ptr = ext.value();
        let cb_ptr = cb_ptr as *mut ModuleLogicSystemCb;
        let system_cb = unsafe { Box::from_raw(cb_ptr) };

        if let Err(ex) = (|| {
            let input: Value = serde_v8::from_v8(scope, args.get(0))
                .map_err(std::io::Error::other)?;

            let output = system_cb(input).map_err(std::io::Error::other)?;

            let output = serde_v8::to_v8(scope, output)
                .map_err(std::io::Error::other)?;

            retval.set(output);

            Result::Ok(())
        })() {
            let ex = v8::String::new(scope, &format!("{ex:?}")).unwrap();
            let ex = v8::Exception::error(scope, ex);
            scope.throw_exception(ex);
        }

        // don't drop our callback yet, this fn may be called multiple times
        // we drop it when the context is shut down.
        std::mem::forget(system_cb);
    }
}

fn v8_thread(
    should_term: Arc<AtomicBool>,
    term_handle_send: tokio::sync::oneshot::Sender<v8::IsolateHandle>,
    mut exec_recv: ExecRecv,
) {
    struct DropGuard(Arc<AtomicBool>);

    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let _g = DropGuard(should_term.clone());

    // only initialize the v8 platform once
    static V8INIT: std::sync::Once = std::sync::Once::new();
    V8INIT.call_once(|| {
        let platform = ::v8::new_default_platform(0, false).make_shared();
        ::v8::V8::initialize_platform(platform);
        ::v8::V8::initialize();
    });

    let isolate = &mut v8::Isolate::new(Default::default());
    let term_handle = isolate.thread_safe_handle();

    if term_handle_send.send(term_handle).is_err() {
        return;
    }

    if should_term.load(Ordering::SeqCst) {
        return;
    }

    let hscope = &mut v8::HandleScope::new(isolate);

    while !should_term.load(Ordering::SeqCst) {
        let Exec { exec, output_send } = match exec_recv.blocking_recv() {
            None => return,
            Some(exec) => exec,
        };

        // convert the callback logic into a v8 external
        // (memory leak if we fail to clean it up later!)
        let system_cb: Box<ModuleLogicSystemCb> = Box::new(exec.system_cb);
        let cb_ptr: *mut ModuleLogicSystemCb = Box::into_raw(system_cb);
        let cb_ptr = cb_ptr as *mut std::ffi::c_void;
        let ext = v8::External::new(hscope, cb_ptr);

        // set up the conext
        let global = v8::ObjectTemplate::new(hscope);

        global.set(
            v8::String::new(hscope, "__VM_SYSTEM").unwrap().into(),
            v8::FunctionTemplate::builder(fn_cb_logic)
                .data(ext.into())
                .build(hscope)
                .into(),
        );

        let context = v8::Context::new(
            hscope,
            v8::ContextOptions {
                global_template: Some(global),
                ..Default::default()
            },
        );

        // set up the scope
        let scope = &mut v8::ContextScope::new(hscope, context);
        let scope = &mut v8::TryCatch::new(scope);

        let res = call_js_fn(scope, exec.logic, exec.input);

        // clean up the cb logic to avoid memory leak!
        let cb_ptr = ext.value();
        let cb_ptr = cb_ptr as *mut ModuleLogicSystemCb;
        let system_cb = unsafe { Box::from_raw(cb_ptr) };
        drop(system_cb);

        let _ = output_send.send(res);
    }
}

const SETUP: &str = r#"
let reg = {
  call: 'register',
  code(input) { throw new Error('VM has not been "register"ed') }
};
globalThis.VM = function VM(input) {
  if (!input || typeof input !== 'object') {
    throw new TypeError('VM(input) must be an object');
  }
  if (!input.call || typeof input.call !== 'string') {
    throw new TypeError('VM(input.call) must be a string');
  }
  if (input.call === 'register') {
    if (typeof input.code !== 'function') {
      throw new TypeError(
        'VM(input.code) must be a function when call is "register"'
      );
    }
    reg = input;
  } else if (input.call === 'validate') {
    return reg.code(input);
  } else if (input.call === 'system') {
    return __VM_SYSTEM(input);
  } else {
    throw new TypeError(`Invalid VM(input.call): "${input.call}"`);
  }
};
globalThis.console = {
    log(...args) {
        globalThis.VM({ call: 'system', type: 'trace', data: args });
    },
    error(...args) {
        globalThis.VM({ call: 'system', type: 'trace', data: args });
    }
};
globalThis.crypto = {
    getRandomValues(a) {
        let b = a;
        if (b.buffer instanceof ArrayBuffer) {
            b = b.buffer;
        }
        if (!(b instanceof ArrayBuffer)) {
            throw new TypeError('getRandomValues expects an ArrayBuffer');
        }
        b = new Uint8Array(b);
        const c = globalThis.VM({
            call: 'system',
            type: 'randomBytes',
            byteLength: b.byteLength
        });
        b.set(c);
        return a;
    }
};
globalThis.TextEncoder = class TextEncoder {
    encode(data) {
        return globalThis.VM({ call: 'system', type: 'utf8Encode', data });
    }
};
globalThis.TextDecoder = class TextDecoder {
    decode(data) {
        return globalThis.VM({ call: 'system', type: 'utf8Decode', data });
    }
};
"#;

fn call_js_fn<'s, 't>(
    scope: &mut v8::TryCatch<'t, v8::HandleScope<'s>>,
    logic: VmLogic,
    input: Value,
) -> Result<Value> {
    // v8-ify the input data
    let input: v8::Local<'_, v8::Value> =
        serde_v8::to_v8(scope, input).map_err(std::io::Error::other)?;

    let mut stage = "v8 init";

    let res: Option<v8::Local<'_, ::v8::Value>> = (|| {
        // import the setup into a v8 string
        let setup = v8::String::new(scope, SETUP)?;

        // compile the code
        stage = "compile setup";
        let script = v8::Script::compile(scope, setup, None)?;

        // execute the code
        stage = "execute setup";
        script.run(scope)?;

        // import the code into a v8 string
        stage = "stringify logic def";
        let code = match logic {
            VmLogic::Utf8Single { code } => code,
        };
        let code = v8::String::new(scope, &code)?;

        // compile the code
        stage = "compile logic def";
        let script = v8::Script::compile(scope, code, None)?;

        // execute the code
        stage = "execute logic def";
        script.run(scope)?;

        // extract the exec_test function
        let global = scope.get_current_context().global(scope);
        stage = "stringify fn_name";
        let call = v8::String::new(scope, "VM")?.into();
        stage = "extract fn reference";
        let fn_ref = global.get_real_named_property(scope, call)?;
        stage = "treat fn reference as a function";
        let fn_ref = v8::Local::<v8::Function>::try_from(fn_ref).ok()?;

        // call the function
        let null = v8::null(scope).into();
        stage = "call function";
        fn_ref.call(scope, null, &[input])
    })();

    let value = res.ok_or_else(|| {
        let info = if scope.has_caught() {
            scope
                .exception()
                .unwrap()
                .to_string(scope)
                .unwrap()
                .to_rust_string_lossy(scope)
        } else {
            "unknown error".into()
        };
        std::io::Error::other(format!("v8 Error at {stage}: {info}"))
    })?;

    serde_v8::from_v8(scope, value).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn v8_sanity() {
        let config = Arc::new(config::Config::default());
        let logic = LogicFactoryV8.factory(config).await.unwrap();

        let mut input = Value::map_new();
        input.map_insert("call".into(), "validate".into());
        input.map_insert("n".into(), Bytes::from_static(b"bob").into());
        input.map_insert("a".into(), 42.0.into());

        let res = logic
            .exec(ModuleLogicExec {
                logic: VmLogic::Utf8Single {
                    code: r#"
VM({
  call: 'register',
  code(input) {
    input.n[0] = 65;
    input.a += 10;
    input.d = '';
    input.d += VM({ call: 'system', data: 'd1' });
    input.d += VM({ call: 'system', data: 'd2' });
    return input;
  }
});"#
                        .into(),
                },
                system_cb: Arc::new(|mut input| {
                    Ok(input.map_remove("data").unwrap())
                }),
                input: input.clone(),
                timeout: None,
            })
            .await
            .unwrap();

        input.map_insert("n".into(), Bytes::from_static(b"Aob").into());
        input.map_insert("a".into(), 52.0.into());
        input.map_insert("d".into(), "d1d2".into());

        assert_eq!(input, res);
    }
}
