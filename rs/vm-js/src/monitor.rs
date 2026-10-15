//! V8/deno_core runs single threaded. While awating a call promise
//! and executing the event loop, interrupting is complicated..
//! You must use the isolate_handle.request_interrupt function.
//! So, for monitoring timeouts on calls, and memory overages,
//! we need a separate thread running to do that monitoring.
//!
//! We create a single monitor thread that lasts for the life of the
//! process, which picks up all jobs from all VmJs instances doing
//! the monitoring as appropriate for those jobs.

use std::collections::HashMap;
use std::sync::Arc;

/// Handle to a running monitor task.
/// Dropping this guard will stop monitoring the given resource.
pub struct MonitorGuard(pub usize);

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        let mon = mon_map().remove(&self.0);
        if let Some(mon) = mon {
            mon.cancel.cancel();
        }
    }
}

/// Register a task to monitor v8 memory usage.
pub fn register_monitor(
    cancel: tokio_util::sync::CancellationToken,
    isolate_handle: deno_core::v8::IsolateHandle,
    max_mem_bytes: usize,
    ab_budget: Arc<crate::alloc::AllocBudget>,
) -> MonitorGuard {
    let uniq = get_uniq();

    mon_map().insert(
        uniq,
        Arc::new(Monitor {
            cancel,
            isolate_handle,
            max_mem_bytes,
            ab_budget,
            start: std::time::Instant::now(),
            timeout_at: std::sync::atomic::AtomicU64::new(0),
            timed_out: std::sync::atomic::AtomicBool::new(false),
        }),
    );

    MonitorGuard(uniq)
}

/// Set up a timeout for a javascript operation.
pub fn set_timeout(mon_uniq: usize, timeout: std::time::Duration) {
    let mon = mon_map().get(&mon_uniq).cloned();
    if let Some(mon) = mon {
        // a fresh operation clears any previous timeout result
        mon.timed_out
            .store(false, std::sync::atomic::Ordering::Relaxed);
        // a pathological duration could overflow the deadline; treat that
        // as "never" rather than panicking
        let deadline = std::time::Instant::now()
            .checked_add(timeout)
            .map(|d| nanos_since(mon.start, d).max(1))
            .unwrap_or(u64::MAX);
        mon.timeout_at
            .store(deadline, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Clear a javascript operation timeout.
pub fn clear_timeout(mon_uniq: usize) {
    let mon = mon_map().get(&mon_uniq).cloned();
    if let Some(mon) = mon {
        mon.timeout_at
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Returns true if the active operation on the given monitor has been
/// terminated by a timeout.
pub fn timed_out(mon_uniq: usize) -> bool {
    let mon = mon_map().get(&mon_uniq).cloned();
    mon.map(|mon| mon.timed_out.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false)
}

/// Mark the given monitor as timed out and tear down its isolate. This is
/// used by the event-loop-level timeout (which can catch parked awaits that
/// the monitor interrupt cannot reach).
pub fn mark_timed_out(mon_uniq: usize) {
    let mon = mon_map().get(&mon_uniq).cloned();
    if let Some(mon) = mon {
        mon.timed_out
            .store(true, std::sync::atomic::Ordering::Relaxed);
        mon.cancel.cancel();
        mon.isolate_handle.terminate_execution();
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
    ab_budget: Arc<crate::alloc::AllocBudget>,
    /// Monotonic origin for `timeout_at`.
    start: std::time::Instant,
    /// Deadline for the active operation, as nanoseconds since `start`.
    /// `0` means no timeout is active. An atomic (rather than a Mutex) so
    /// the interrupt callback can read it without locking.
    timeout_at: std::sync::atomic::AtomicU64,
    /// Latched true when the current operation timed out. Set by the
    /// monitor interrupt for CPU-bound code, or by `mark_timed_out` for
    /// parked awaits. Lets the caller report a clear, fatal timeout error
    /// instead of a generic "execution terminated" non-fatal error.
    timed_out: std::sync::atomic::AtomicBool,
}

/// Nanoseconds from `start` to `t`, saturating instead of overflowing.
fn nanos_since(start: std::time::Instant, t: std::time::Instant) -> u64 {
    t.saturating_duration_since(start)
        .as_nanos()
        .min(u64::MAX as u128) as u64
}

/// Returns true if the operation associated with `timeout_at` has passed
/// its deadline. A stored value of `0` means no timeout is active.
fn is_timed_out(
    start: std::time::Instant,
    timeout_at: &std::sync::atomic::AtomicU64,
) -> bool {
    let deadline = timeout_at.load(std::sync::atomic::Ordering::Relaxed);
    deadline != 0 && nanos_since(start, std::time::Instant::now()) >= deadline
}

/// Access the map containing active js threads to monitor.
///
/// This is only touched outside of javascript execution: registration and
/// teardown, `set_timeout`/`clear_timeout`/`timed_out`/`mark_timed_out`, and
/// the monitor thread. The interrupt callback (`mem_interrupt_cb`) must NOT
/// call this; it receives its `&Monitor` directly through the interrupt data
/// pointer so that it stays lock-free.
///
/// When iterating, collect/clone the entries so the lock isn't held while
/// performing operations.
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

        let list: Vec<Arc<Monitor>> = mon_map().values().cloned().collect();

        // for the complete list of set up monitors
        for mon in list {
            // request an interrupt in the js engine so we can
            // check some state, and shut down execution if needed.
            // hand the monitor to the callback directly so it needs
            // no global lookup (and no lock) of its own.
            mon.isolate_handle.request_interrupt(
                mem_interrupt_cb,
                Arc::as_ptr(&mon) as *mut std::ffi::c_void,
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

    // SAFETY: `data` is a `*const Monitor` handed to `request_interrupt` by
    // the monitor thread (see `mon_thread`). Interrupt callbacks only run on
    // the isolate thread while it is executing javascript, and the monitor
    // map keeps the `Arc<Monitor>` alive for the entire lifetime of that
    // thread's call loop, so the pointer is valid whenever a callback can
    // run. `request_interrupt` on a destroyed isolate is a no-op and any
    // queued interrupt is discarded when the isolate is torn down.
    let mon = unsafe { &*(data as *const Monitor) };

    // if we've already been cancelled, we can exit early
    if mon.cancel.is_cancelled() {
        isolate.terminate_execution();
        return;
    }

    // if an active call has timed out, that is fatal
    // because the call could have zombie promises that would
    // infect future calls
    if is_timed_out(mon.start, &mon.timeout_at) {
        mon.timed_out
            .store(true, std::sync::atomic::Ordering::Relaxed);
        mon.cancel.cancel();
        isolate.terminate_execution();
        return;
    }

    // if the arraybuffer allocator has already rejected an allocation,
    // the isolate is over its quota and must be torn down
    if mon.ab_budget.is_over() {
        mon.cancel.cancel();
        isolate.terminate_execution();
        return;
    }

    // finally check to see if we are over on our memory quota
    let stats = isolate.get_heap_statistics();
    let ab_used = mon.ab_budget.used();
    let total = stats.total_heap_size() + ab_used;

    if total > mon.max_mem_bytes {
        mon.cancel.cancel();
        isolate.terminate_execution();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::{Duration, Instant};

    #[test]
    fn nanos_since_is_monotonic_and_saturating() {
        let start = Instant::now();
        let later = start + Duration::from_millis(5);

        assert_eq!(nanos_since(start, start), 0);
        assert_eq!(nanos_since(start, later), 5_000_000);
        // a time before `start` saturates to 0 rather than underflowing
        assert_eq!(nanos_since(later, start), 0);
    }

    #[test]
    fn is_timed_out_handles_none_and_deadlines() {
        let start = Instant::now();

        // 0 means "no active timeout" and must never fire
        let none = AtomicU64::new(0);
        assert!(!is_timed_out(start, &none));

        // a deadline far in the future does not fire
        let future = AtomicU64::new(
            nanos_since(start, start + Duration::from_secs(3600)).max(1),
        );
        assert!(!is_timed_out(start, &future));

        // the overflow sentinel (used for pathological durations) never fires
        let never = AtomicU64::new(u64::MAX);
        assert!(!is_timed_out(start, &never));

        // a deadline in the past fires
        let past = AtomicU64::new(1);
        std::thread::sleep(Duration::from_millis(2));
        assert!(is_timed_out(start, &past));

        // clearing the deadline disarms it again
        past.store(0, std::sync::atomic::Ordering::Relaxed);
        assert!(!is_timed_out(start, &past));
    }
}
