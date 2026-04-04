use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Handle to a running monitor task.
/// Dropping this guard will stop monitoring the given resource.
pub struct MonitorGuard(pub usize);

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        if let Some(mon) = mon_map().remove(&self.0) {
            mon.cancel.cancel();
            mon.isolate_handle.terminate_execution();
        }
    }
}

/// Register a task to monitor v8 memory usage.
pub fn register_monitor(
    cancel: tokio_util::sync::CancellationToken,
    isolate_handle: deno_core::v8::IsolateHandle,
    max_mem_bytes: usize,
    ab_bytes: Arc<std::sync::atomic::AtomicUsize>,
) -> MonitorGuard {
    let uniq = get_uniq();
    mon_map().insert(
        uniq,
        Arc::new(Monitor {
            cancel,
            isolate_handle,
            max_mem_bytes,
            ab_bytes,
            timeout_at: Mutex::new(None),
        }),
    );

    MonitorGuard(uniq)
}

/// Set up a timeout for a javascript operation.
pub fn set_timeout(mon_uniq: usize, timeout: std::time::Duration) {
    if let Some(mon) = mon_map().get(&mon_uniq) {
        *mon.timeout_at.lock().unwrap() =
            Some(std::time::Instant::now() + timeout);
    }
}

/// Clear a javascript operation timeout.
pub fn clear_timeout(mon_uniq: usize) {
    if let Some(mon) = mon_map().get(&mon_uniq) {
        *mon.timeout_at.lock().unwrap() = None;
    }
}

fn get_uniq() -> usize {
    static MON_UNIQ: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(1);
    MON_UNIQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

struct Monitor {
    cancel: tokio_util::sync::CancellationToken,
    isolate_handle: deno_core::v8::IsolateHandle,
    max_mem_bytes: usize,
    ab_bytes: Arc<std::sync::atomic::AtomicUsize>,
    timeout_at: Mutex<Option<std::time::Instant>>,
}

fn mon_map() -> std::sync::MutexGuard<'static, HashMap<usize, Arc<Monitor>>> {
    static MON_MAP: std::sync::OnceLock<
        std::sync::Mutex<HashMap<usize, Arc<Monitor>>>,
    > = std::sync::OnceLock::new();
    MON_MAP
        .get_or_init(|| {
            // spawn this single global thread for monitoring
            let _ = std::thread::spawn(mon_thread);
            Default::default()
        })
        .lock()
        .unwrap()
}

fn mon_thread() {
    loop {
        // anything shorter could cause thrashing in the js execution
        // we could potentially go as high as 500ms but then timeouts
        // start to feel untimely, and we might leave memory overages
        // up for longer...
        std::thread::sleep(std::time::Duration::from_millis(100));

        let list: Vec<(usize, Arc<Monitor>)> = mon_map()
            .iter()
            .map(|(uniq, mon)| (*uniq, mon.clone()))
            .collect();

        // for the complete list of set up monitors
        for (uniq, mon) in list {
            // request an interrupt in the js engine so we can
            // check some state, and shut down execution if needed
            mon.isolate_handle.request_interrupt(
                mem_interrupt_cb,
                uniq as *mut std::ffi::c_void,
            );
        }
    }
}

unsafe extern "C" fn mem_interrupt_cb(
    mut isolate: deno_core::v8::UnsafeRawIsolatePtr,
    data: *mut std::ffi::c_void,
) {
    // type our inputs correctly
    let isolate = unsafe {
        deno_core::v8::Isolate::ref_from_raw_isolate_ptr_mut(&mut isolate)
    };
    let uniq: usize = data as usize;

    // access the registered monitor data
    let mon = mon_map().get(&uniq).cloned();
    let mon = match mon {
        // if this doesn't exist, the thread is already shutting down
        // we can safely do nothing
        None => return,
        Some(mon) => mon,
    };

    // if we've already been cancelled, we can exit early
    if mon.cancel.is_cancelled() {
        isolate.terminate_execution();
        return;
    }

    // if an active call has timed out, that is fatal
    // because the call could have zombie promises that would
    // infect future calls
    let now = std::time::Instant::now();
    if let Some(timeout_at) = *mon.timeout_at.lock().unwrap()
        && timeout_at <= now
    {
        mon.cancel.cancel();
        isolate.terminate_execution();
        return;
    }

    // finally check to see if we are over on our memory quota
    let stats = isolate.get_heap_statistics();
    let ab_used = mon.ab_bytes.load(std::sync::atomic::Ordering::Relaxed);
    let total = stats.used_heap_size() + ab_used;

    if total > mon.max_mem_bytes {
        mon.cancel.cancel();
        isolate.terminate_execution();
    }
}
