//! vm-js: Void Merge Javascript Engine.

use std::io::Result;
use std::sync::Arc;

mod alloc;
mod js_thread;
mod monitor;

/// Void Merge Javascript Engine Configuration.
#[derive(Clone)]
pub struct VmJsConfig {
    /// The javascript code to load.
    pub code: Arc<str>,

    /// The memory usage limit in bytes.
    pub max_mem_bytes: usize,

    /// Idle thread shutdown duration.
    pub idle_shutdown: std::time::Duration,
}

impl Default for VmJsConfig {
    fn default() -> Self {
        Self {
            code: "".into(),
            max_mem_bytes: 1024 * 1024 * 32,
            idle_shutdown: std::time::Duration::from_secs(120),
        }
    }
}

/// Void Merge Javascript Engine.
pub struct VmJs<Input, Output>
where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    config: VmJsConfig,
    _p: std::marker::PhantomData<(Input, Output)>,
}

impl<Input, Output> VmJs<Input, Output>
where
    Input: 'static + Send + serde::Serialize,
    Output: 'static + Send + serde::de::DeserializeOwned,
{
    /// Construct a new [VmJs] instance.
    pub async fn new(config: VmJsConfig) -> Result<Self> {
        Ok(VmJs {
            config,
            _p: std::marker::PhantomData,
        })
    }

    /// Call an async javascript function.
    pub async fn call(
        &self,
        _fn_name: String,
        _input: Input,
    ) -> Result<Output> {
        let (_call_send, call_recv) = tokio::sync::mpsc::channel(32);
        let config = self.config.clone();
        let _thread = std::thread::spawn(move || {
            js_thread::js_thread_loop::<Input, Output>(config, call_recv)
        });
        todo!()
    }
}
