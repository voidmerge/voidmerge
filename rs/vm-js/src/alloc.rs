//! V8 only tracks internal javascript memory which doesn't include
//! ArrayBuffers like Uint8Array instances. Those are managed by
//! rust. So we need to keep track of that as well. This custom
//! allocator allows us to do that tracking.
//!
//! In addition to tracking, the allocator enforces a hard quota at
//! allocation time. Without this, a single `new Uint8Array(...)`
//! statement could allocate far past the configured limit before the
//! asynchronous monitor thread noticed (the monitor only samples every
//! 100ms). Rejecting the allocation up-front keeps a hostile script from
//! ballooning the process and getting it OOM-killed.

use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

/// alignment 8 could be overly permissive: V8's ArrayBuffer API
/// doesn't require alignment stronger than 8 bytes for typed arrays,
/// so this is fine in practice.
const ALIGN: usize = 8;

/// Shared accounting for the ArrayBuffer allocator.
///
/// `used` is the number of live bytes currently handed out, `limit` is the
/// quota, and `over` is latched once a reservation has been rejected so the
/// monitor thread can tear down the isolate.
pub struct AllocBudget {
    used: atomic::AtomicUsize,
    limit: usize,
    over: AtomicBool,
}

impl AllocBudget {
    fn new(limit: usize) -> Self {
        Self {
            used: atomic::AtomicUsize::new(0),
            limit,
            over: AtomicBool::new(false),
        }
    }

    /// Current live ArrayBuffer bytes.
    pub fn used(&self) -> usize {
        self.used.load(atomic::Ordering::Relaxed)
    }

    /// Whether an allocation has been rejected because it would exceed the
    /// quota. Once true, this stays true for the life of the budget.
    pub fn is_over(&self) -> bool {
        self.over.load(atomic::Ordering::Relaxed)
    }

    /// Atomically reserve `len` bytes against the quota. Returns false (and
    /// latches `over`) if the reservation would exceed the quota.
    fn reserve(&self, len: usize) -> bool {
        let ok = self
            .used
            .fetch_update(
                atomic::Ordering::Relaxed,
                atomic::Ordering::Relaxed,
                |cur| match cur.checked_add(len) {
                    Some(next) if next <= self.limit => Some(next),
                    _ => None,
                },
            )
            .is_ok();
        if !ok {
            self.over.store(true, atomic::Ordering::Relaxed);
        }
        ok
    }

    /// Release previously reserved bytes.
    fn release(&self, len: usize) {
        self.used.fetch_sub(len, atomic::Ordering::Relaxed);
    }
}

unsafe extern "C" fn ab_alloc_zeroed(
    handle: &AllocBudget,
    len: usize,
) -> *mut c_void {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    }
    let layout = match Layout::from_size_align(len, ALIGN) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    // reserve against the quota before touching the system allocator so a
    // single large allocation can't overshoot the limit
    if !handle.reserve(len) {
        return std::ptr::null_mut();
    }
    let ptr = unsafe { alloc_zeroed(layout) };
    if ptr.is_null() {
        handle.release(len);
        return std::ptr::null_mut();
    }
    ptr.cast()
}

unsafe extern "C" fn ab_alloc_uninit(
    handle: &AllocBudget,
    len: usize,
) -> *mut c_void {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    }
    let layout = match Layout::from_size_align(len, ALIGN) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    // reserve against the quota before touching the system allocator so a
    // single large allocation can't overshoot the limit
    if !handle.reserve(len) {
        return std::ptr::null_mut();
    }
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        handle.release(len);
        return std::ptr::null_mut();
    }
    ptr.cast()
}

unsafe extern "C" fn ab_free(
    handle: &AllocBudget,
    data: *mut c_void,
    len: usize,
) {
    if len == 0 {
        return;
    }
    let layout = match Layout::from_size_align(len, ALIGN) {
        Ok(l) => l,
        Err(_) => return, // Unreachable if alloc succeeded
    };
    unsafe { dealloc(data.cast(), layout) };
    handle.release(len);
}

unsafe extern "C" fn ab_drop(handle: *const AllocBudget) {
    // Reconstruct the Arc we leaked in new_tracking_allocator() and drop it.
    drop(unsafe { Arc::from_raw(handle) });
}

static AB_VTABLE: deno_core::v8::RustAllocatorVtable<AllocBudget> =
    deno_core::v8::RustAllocatorVtable {
        allocate: ab_alloc_zeroed,
        allocate_uninitialized: ab_alloc_uninit,
        free: ab_free,
        drop: ab_drop,
    };

/// Returns an `Arc<AllocBudget>` that tracks and enforces live ArrayBuffer
/// bytes, and a V8 allocator wired to it. Pass the allocator to
/// `CreateParams` and keep the `Arc` to query usage from Rust.
///
/// `limit` is the maximum number of ArrayBuffer bytes allowed to be live at
/// once. Allocations that would exceed it are rejected.
pub fn new_tracking_allocator(
    limit: usize,
) -> (
    Arc<AllocBudget>,
    deno_core::v8::UniqueRef<deno_core::v8::Allocator>,
) {
    let budget = Arc::new(AllocBudget::new(limit));
    // Safety: we pass a raw pointer to the Arc's inner value and a matching
    // 'static vtable. ab_drop reconstructs the Arc and drops it when V8 is
    // finished with the allocator.
    let allocator = unsafe {
        deno_core::v8::new_rust_allocator(
            Arc::into_raw(budget.clone()),
            &AB_VTABLE,
        )
    };
    (budget, allocator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_enforces_limit() {
        let b = AllocBudget::new(100);
        assert!(b.reserve(60));
        assert_eq!(b.used(), 60);
        assert!(!b.is_over());

        assert!(b.reserve(40));
        assert_eq!(b.used(), 100);
        assert!(!b.is_over());

        // a single allocation over the remaining quota is rejected and
        // does not change the accounted usage
        assert!(!b.reserve(1));
        assert_eq!(b.used(), 100);
        assert!(b.is_over());

        // releasing frees room again
        b.release(100);
        assert_eq!(b.used(), 0);
    }

    #[test]
    fn reserve_rejects_single_oversized_alloc() {
        let b = AllocBudget::new(16 * 1024 * 1024);
        assert!(!b.reserve(256 * 1024 * 1024));
        assert_eq!(b.used(), 0);
        assert!(b.is_over());
    }

    #[test]
    fn reserve_handles_overflow() {
        let b = AllocBudget::new(usize::MAX);
        assert!(b.reserve(usize::MAX));
        assert!(!b.reserve(1));
        assert!(b.is_over());
    }
}
