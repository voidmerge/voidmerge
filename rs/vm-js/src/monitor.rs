use std::sync::Arc;

pub struct Monitor {}

impl Drop for Monitor {
    fn drop(&mut self) {}
}

impl Monitor {
    pub fn new(
        _isolate_handle: deno_core::v8::IsolateHandle,
        _max_mem_bytes: usize,
        _ab_bytes: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        Self {}
    }
}
