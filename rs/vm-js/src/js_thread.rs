use crate::{JsError, JsResult};
use JsError::*;
use deno_core::v8;
use std::sync::Arc;

pub type FnName = &'static str;

pub enum Call<Input, Output> {
    Call {
        fn_name: FnName,
        input: Input,
        resp: tokio::sync::oneshot::Sender<JsResult<Output>>,
    },
}

type CallRecv<Input, Output> = tokio::sync::mpsc::Receiver<Call<Input, Output>>;

pub fn js_thread_loop<Input, Output>(
    config: crate::VmJsConfig,
    cancel: tokio_util::sync::CancellationToken,
    call_recv: CallRecv<Input, Output>,
) where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        tokio::select! {
            _ = cancel.cancelled() => (),
            _ = js_thread_loop_async::<Input, Output>(config, call_recv) => (),
        }
    });
}

pub async fn js_thread_loop_async<Input, Output>(
    config: crate::VmJsConfig,
    mut call_recv: CallRecv<Input, Output>,
) where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    let (ab_bytes, ab_allocator) = crate::alloc::new_tracking_allocator();

    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        create_params: Some(
            v8::CreateParams::default()
                .heap_limits(1, config.max_mem_bytes)
                .array_buffer_allocator(ab_allocator),
        ),
        extensions: vec![
            /*
            deno_webidl::deno_webidl::init(),
            deno_web::deno_web::init(
                Arc::new(deno_web::BlobStore::default()),
                None, // location
                deno_web::InMemoryBroadcastChannel::default(),
            ),
            deno_node_stub,
            deno_crypto::deno_crypto::init(None),
            bootstrap_ext,
            spike::init(),
            */
        ],
        ..Default::default()
    });

    let isolate_handle = js_runtime.v8_isolate().thread_safe_handle();
    js_runtime.add_near_heap_limit_callback(move |cur, _init| {
        // the monitor thread manages the true memory usage
        // including our arraybuffers.
        // this is a fallback incase the memory usage increases
        // just in the heap faster than the monitor check interval
        isolate_handle.terminate_execution();

        // we will terminate, but don't want a crash in the mean time
        cur * 2
    });

    let mut did_setup = false;

    while let Some(call) = call_recv.recv().await {
        let (fn_name, input, resp) = match call {
            Call::Call {
                fn_name,
                input,
                resp,
            } => (fn_name, input, resp),
        };

        // check setup
        if !did_setup {
            did_setup = true;

            if let Err(err) =
                js_runtime.execute_script("<setup>", config.code.clone())
            {
                let err = std::io::Error::other(format!(
                    "failed to load javascript code: {err:?}"
                ));
                let _ = resp.send(Err(JsError::Fatal(err)));
                return;
            }
        }

        match exec_call(&mut js_runtime, &config, &ab_bytes, fn_name, input)
            .await
        {
            Ok(output) => {
                let _ = resp.send(Ok(output));
            }
            Err(NonFatal(err)) => {
                let _ = resp.send(Err(NonFatal(err)));
                continue;
            }
            Err(Fatal(err)) => {
                let _ = resp.send(Err(Fatal(err)));
                return;
            }
        }
    }
}

async fn exec_call<Input, Output>(
    js_runtime: &mut deno_core::JsRuntime,
    config: &crate::VmJsConfig,
    ab_bytes: &Arc<std::sync::atomic::AtomicUsize>,
    fn_name: FnName,
    input: Input,
) -> JsResult<Output>
where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    // Extract jsFn as a typed function binding from globalThis
    let js_fn: v8::Global<v8::Function> = {
        let ctx = js_runtime.main_context();
        v8::scope_with_context!(scope, js_runtime.v8_isolate(), ctx);
        let global_obj = scope.get_current_context().global(scope);
        let key = v8::String::new(scope, fn_name).ok_or(NonFatal(
            std::io::Error::other("failed import v8 fn_name str"),
        ))?;
        let val = global_obj
            .get(scope, key.into())
            .ok_or(NonFatal(std::io::Error::other("fn_name not on global")))?;
        let func = v8::Local::<v8::Function>::try_from(val).map_err(
            JsError::non_fatal("fn_name undefined or not a function"),
        )?;
        v8::Global::new(scope, func)
    };

    // Build the call argument
    let input: v8::Global<v8::Value> = {
        let ctx = js_runtime.main_context();
        v8::scope_with_context!(scope, js_runtime.v8_isolate(), ctx);
        let input: v8::Local<v8::Value> = serde_v8::to_v8(scope, input)
            .map_err(JsError::non_fatal("serializing input to v8"))?;
        v8::Global::new(scope, input)
    };

    // Set up call memory monitoring
    let isolate_handle = js_runtime.v8_isolate().thread_safe_handle();
    let mon_g = crate::monitor::register_monitor(
        isolate_handle,
        config.max_mem_bytes,
        ab_bytes.clone(),
    );

    // Call via typed binding; drive the event loop while the async fn runs
    let call = js_runtime.call_with_args(&js_fn, &[input]);
    let event_loop_result = js_runtime
        .with_event_loop_promise(call, Default::default())
        .await;

    // stop the memory monitoring
    drop(mon_g);

    let output = match event_loop_result {
        Ok(output) => output,
        Err(err) => match err.into_kind() {
            deno_core::error::CoreErrorKind::Js(err) => {
                return Err(JsError::non_fatal("javascript execution error")(
                    err,
                ));
            }
            deno_core::error::CoreErrorKind::JsBox(err) => {
                return Err(JsError::non_fatal("javascript execution error")(
                    err,
                ));
            }
            deno_core::error::CoreErrorKind::Io(err) => {
                return Err(JsError::non_fatal("javascript io error")(err));
            }
            deno_core::error::CoreErrorKind::Data(err) => {
                return Err(JsError::non_fatal("javascript data error")(err));
            }
            deno_core::error::CoreErrorKind::Url(err) => {
                return Err(JsError::non_fatal("javascript url error")(err));
            }
            // NOTE - more of these deno_errors may be non-fatal
            //        if so, they should be moved above this comment
            //        all other errors are treated as fatal
            //        and this isolate thread must shut down
            err => {
                return Err(JsError::fatal("error executing v8 call")(err));
            }
        },
    };

    let output = {
        let ctx = js_runtime.main_context();
        v8::scope_with_context!(scope, js_runtime.v8_isolate(), ctx);
        let output = v8::Local::new(scope, output);
        serde_v8::from_v8(scope, output)
            .map_err(JsError::non_fatal("deserializing v8 output"))?
    };

    Ok(output)
}
