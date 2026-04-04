//! vm-js: Void Merge Javascript Engine.

use std::sync::Arc;

mod alloc;
mod js_thread;
mod monitor;

#[cfg(test)]
mod test;

/// Javascript error indicating if the engine is still viable
/// or must be dropped / recreated.
#[derive(Debug, thiserror::Error)]
pub enum JsError {
    /// The javascript thread had to shut down, it is no longer usable.
    #[error(transparent)]
    Fatal(std::io::Error),

    /// An error with the particular request. You can still use this thread.
    #[error(transparent)]
    NonFatal(std::io::Error),
}

use JsError::*;

impl JsError {
    pub fn fatal<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
        info: impl Into<Arc<str>>,
    ) -> impl FnOnce(E) -> JsError {
        move |err| {
            Fatal(std::io::Error::other(WithInfo(info.into(), err.into())))
        }
    }

    pub fn non_fatal<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
        info: impl Into<Arc<str>>,
    ) -> impl FnOnce(E) -> JsError {
        move |err| {
            NonFatal(std::io::Error::other(WithInfo(info.into(), err.into())))
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}: {1}")]
struct WithInfo(Arc<str>, #[source] Box<dyn std::error::Error + Send + Sync>);

/// Javascript result type.
pub type JsResult<T> = std::result::Result<T, JsError>;

/// Void Merge Javascript Engine Configuration.
#[derive(Clone)]
pub struct VmJsConfig {
    /// The javascript code to load.
    pub code: Arc<str>,

    /// The memory usage limit in bytes.
    pub max_mem_bytes: usize,
}

impl Default for VmJsConfig {
    fn default() -> Self {
        Self {
            code: "".into(),
            max_mem_bytes: 1024 * 1024 * 32,
        }
    }
}

/// Void Merge Javascript Engine.
pub struct VmJs<Input, Output>
where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    cancel: tokio_util::sync::CancellationToken,
    thread: Option<std::thread::JoinHandle<()>>,
    call_send: tokio::sync::mpsc::Sender<js_thread::Call<Input, Output>>,
}

impl<Input, Output> Drop for VmJs<Input, Output>
where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    fn drop(&mut self) {
        self.cancel.cancel();
        /* .. not sure we want to await this inline... might slow tokio
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        */
    }
}

impl<Input, Output> VmJs<Input, Output>
where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    /// Construct a new [VmJs] instance.
    pub async fn new(config: VmJsConfig) -> JsResult<Self> {
        let cancel = tokio_util::sync::CancellationToken::new();
        let (call_send, call_recv) = tokio::sync::mpsc::channel(32);
        let cancel2 = cancel.clone();
        let (ready_send, ready_recv) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            js_thread::js_thread_loop::<Input, Output>(
                config, cancel2, call_recv, ready_send,
            )
        });

        let _ = ready_recv.await.map_err(|_| {
            JsError::Fatal(std::io::Error::other("thread setup failed"))
        })?;

        Ok(VmJs {
            cancel,
            thread: Some(thread),
            call_send,
        })
    }

    /// Shut down the javascript engine, running a blocking join on the thread.
    pub fn blocking_shutdown(mut self) {
        self.cancel.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    /// Call an async javascript function.
    pub async fn call(
        &self,
        fn_name: &'static str,
        input: Input,
        timeout: std::time::Duration,
    ) -> JsResult<Output> {
        match self.call_err(fn_name, input, timeout).await {
            Err(Fatal(err)) => {
                self.cancel.cancel();
                Err(Fatal(err))
            }
            oth => oth,
        }
    }

    async fn call_err(
        &self,
        fn_name: &'static str,
        input: Input,
        timeout: std::time::Duration,
    ) -> JsResult<Output> {
        if self.cancel.is_cancelled() {
            return Err(JsError::Fatal(std::io::Error::other(
                "VmJs has shut down",
            )));
        }
        let (s, r) = tokio::sync::oneshot::channel();
        if let Err(_) = self
            .call_send
            .send(js_thread::Call::Call {
                fn_name,
                input,
                resp: s,
                timeout,
            })
            .await
        {
            return Err(JsError::Fatal(std::io::Error::other(
                "VmJs has shut down",
            )));
        }
        r.await.map_err(|_| {
            JsError::Fatal(std::io::Error::other("VmJs has shut down"))
        })?
    }
}
