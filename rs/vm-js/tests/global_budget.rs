//! Integration test for the process-wide budget setters.
//!
//! This lives in its own integration-test binary so it gets its own process
//! and can safely configure the process-global budget without racing the
//! unit tests (which share a process and use injected budgets instead).

use std::time::Duration;

use vm_js::{
    VmJs, VmJsConfig, set_global_max_ram_bytes, set_global_max_threads,
};

fn mk_config() -> VmJsConfig {
    VmJsConfig {
        code: "function bob(a) { return a + 1; }".into(),
        max_mem_bytes: 16 * 1024 * 1024,
        // short wait so the blocked case fails fast rather than hanging
        global_acquire_timeout: Duration::from_millis(200),
        ..Default::default()
    }
}

#[tokio::test]
async fn process_global_thread_cap() {
    // must be set before the first VmJs initializes the global budget
    assert!(set_global_max_threads(1));
    assert!(set_global_max_ram_bytes(64 * 1024 * 1024));

    let a: VmJs<usize, usize> = VmJs::new(mk_config()).await.unwrap();

    // the one thread permit is held; a second instance must fail to acquire
    let err = VmJs::<usize, usize>::new(mk_config()).await.err().unwrap();
    assert!(err.to_string().contains("timed out"), "{err:?}");

    // releasing the first lets a new one through
    drop(a);
    let b: VmJs<usize, usize> = VmJs::new(mk_config()).await.unwrap();
    b.blocking_shutdown();
}
