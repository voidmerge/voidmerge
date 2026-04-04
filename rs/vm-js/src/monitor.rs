use std::collections::HashMap;
use std::sync::Arc;

/// Handle to a running monitor task.
/// Dropping this guard will stop monitoring the given resource.
pub struct MonitorGuard(usize);

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        mon_map().remove(&self.0);
    }
}

/// Register a task to monitor v8 memory usage.
pub fn register_monitor(
    isolate_handle: deno_core::v8::IsolateHandle,
    max_mem_bytes: usize,
    ab_bytes: Arc<std::sync::atomic::AtomicUsize>,
) -> MonitorGuard {
    let uniq = get_uniq();
    mon_map().insert(
        uniq,
        Arc::new(Monitor {
            isolate_handle,
            max_mem_bytes,
            ab_bytes,
        }),
    );

    MonitorGuard(uniq)
}

fn get_uniq() -> usize {
    static MON_UNIQ: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(1);
    MON_UNIQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

struct Monitor {
    isolate_handle: deno_core::v8::IsolateHandle,
    max_mem_bytes: usize,
    ab_bytes: Arc<std::sync::atomic::AtomicUsize>,
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
        std::thread::sleep(std::time::Duration::from_millis(100));

        let list: Vec<(usize, Arc<Monitor>)> = mon_map()
            .iter()
            .map(|(uniq, mon)| (*uniq, mon.clone()))
            .collect();

        for (uniq, mon) in list {
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
    let isolate = unsafe {
        deno_core::v8::Isolate::ref_from_raw_isolate_ptr_mut(&mut isolate)
    };
    let uniq: usize = data as usize;

    let mon = mon_map().get(&uniq).cloned();
    let mon = match mon {
        None => return,
        Some(mon) => mon,
    };

    let stats = isolate.get_heap_statistics();
    let ab_used = mon.ab_bytes.load(std::sync::atomic::Ordering::Relaxed);
    let total = stats.used_heap_size() + ab_used;

    if total > mon.max_mem_bytes {
        isolate.terminate_execution();
    }
}
