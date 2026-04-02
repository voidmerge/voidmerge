#[derive(Debug, thiserror::Error)]
pub enum JsThreadError {
    /// The javascript thread had to shut down, it is no longer usable.
    #[error(transparent)]
    Fatal(std::io::Error),

    /// An error with the particular request. You can still use this thread.
    #[error(transparent)]
    NonFatal(std::io::Error),
}

pub type JsThreadResult<T> = std::result::Result<T, JsThreadError>;

type Call<Input, Output> =
    (Input, tokio::sync::oneshot::Sender<JsThreadResult<Output>>);

type CallRecv<Input, Output> = tokio::sync::mpsc::Receiver<Call<Input, Output>>;

pub fn js_thread_loop<Input, Output>(
    config: crate::VmJsConfig,
    call_recv: CallRecv<Input, Output>,
) where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(js_thread_loop_async::<Input, Output>(config, call_recv));
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
            deno_core::v8::CreateParams::default()
                //.heap_limits(1, MAX_HEAP)
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

    let mut did_setup = false;

    while let Some((_input, resp)) = call_recv.recv().await {
        if !did_setup {
            did_setup = true;

            if let Err(err) = js_runtime
                .execute_script("<setup>", config.code.clone())
                .map_err(std::io::Error::other)
            {
                let _ = resp.send(Err(JsThreadError::Fatal(err)));
                return;
            }
        }

        let isolate_handle = js_runtime.v8_isolate().thread_safe_handle();
        let mon = crate::monitor::Monitor::new(
            isolate_handle,
            config.max_mem_bytes,
            ab_bytes.clone(),
        );

        // TODO - exec code

        drop(mon);
    }
}
