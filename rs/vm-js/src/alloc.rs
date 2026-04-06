//! V8 only tracks internal javascript memory which doesn't include
//! ArrayBuffers like Uint8Array instances. Those are managed by
//! rust. So we need to keep track of that as well. This custom
//! allocator allows us to do that tracking.

use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic;

/// alignment 8 could be overly permissive: V8's ArrayBuffer API
/// doesn't require alignment stronger than 8 bytes for typed arrays,
/// so this is fine in practice.
const ALIGN: usize = 8;

unsafe extern "C" fn ab_alloc_zeroed(
    handle: &atomic::AtomicUsize,
    len: usize,
) -> *mut c_void {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    }
    let layout = match Layout::from_size_align(len, ALIGN) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    let ptr = unsafe { alloc_zeroed(layout) };
    if !ptr.is_null() {
        handle.fetch_add(len, atomic::Ordering::Relaxed);
    }
    ptr.cast()
}

unsafe extern "C" fn ab_alloc_uninit(
    handle: &atomic::AtomicUsize,
    len: usize,
) -> *mut c_void {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    }
    let layout = match Layout::from_size_align(len, ALIGN) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    let ptr = unsafe { alloc(layout) };
    if !ptr.is_null() {
        handle.fetch_add(len, atomic::Ordering::Relaxed);
    }
    ptr.cast()
}

unsafe extern "C" fn ab_free(
    handle: &atomic::AtomicUsize,
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
    handle.fetch_sub(len, atomic::Ordering::Relaxed);
}

unsafe extern "C" fn ab_drop(handle: *const atomic::AtomicUsize) {
    // Reconstruct the Arc we leaked in new_tracking_allocator() and drop it.
    drop(unsafe { Arc::from_raw(handle) });
}

static AB_VTABLE: deno_core::v8::RustAllocatorVtable<atomic::AtomicUsize> =
    deno_core::v8::RustAllocatorVtable {
        allocate: ab_alloc_zeroed,
        allocate_uninitialized: ab_alloc_uninit,
        free: ab_free,
        drop: ab_drop,
    };

/// Returns an `Arc<AtomicUsize>` that tracks live ArrayBuffer bytes, and a
/// V8 allocator wired to it. Pass the allocator to `CreateParams` and keep
/// the `Arc` to query the byte count from Rust.
pub fn new_tracking_allocator() -> (
    Arc<atomic::AtomicUsize>,
    deno_core::v8::UniqueRef<deno_core::v8::Allocator>,
) {
    let bytes = Arc::new(atomic::AtomicUsize::new(0));
    // Safety: we pass a raw pointer to the Arc's inner value and a matching
    // 'static vtable. ab_drop reconstructs the Arc and drops it when V8 is
    // finished with the allocator.
    let allocator = unsafe {
        deno_core::v8::new_rust_allocator(
            Arc::into_raw(bytes.clone()),
            &AB_VTABLE,
        )
    };
    (bytes, allocator)
}
