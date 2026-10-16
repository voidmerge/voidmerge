//! Process-wide resource budget shared across all [crate::VmJs] instances.
//!
//! Each `VmJs` owns a dedicated OS thread and a V8 isolate with its own
//! per-instance memory quota. On its own that bounds a single client, but
//! nothing bounds the *aggregate*: N clients could each take their full
//! quota and exhaust the host. This module reintroduces the process-wide
//! thread and RAM limits the older `voidmerge` implementation had.
//!
//! A [VmJsGlobal] holds two semaphores: one counting OS threads and one
//! counting RAM in mebibytes (MiB). Creating a `VmJs` acquires a thread
//! permit and a RAM permit (rounded up from its `max_mem_bytes`) and holds
//! them for the instance's lifetime, releasing them when it is dropped.
//!
//! RAM is accounted in whole MiB because [`tokio::sync::Semaphore`] takes a
//! `u32` permit count and byte-granular permits would overflow for any
//! budget above 4 GiB.

use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use crate::{JsError, JsResult};

/// Bytes in one mebibyte.
const MIB: usize = 1024 * 1024;

/// Process-wide override for the thread cap, set via
/// [set_global_max_threads].
static MAX_THREADS: OnceLock<usize> = OnceLock::new();

/// Process-wide override for the RAM budget in bytes, set via
/// [set_global_max_ram_bytes].
static MAX_RAM_BYTES: OnceLock<usize> = OnceLock::new();

/// Set the process-wide maximum number of concurrent `VmJs` threads.
///
/// This only takes effect if called before the first `VmJs` is created
/// (i.e. before the process-global budget is initialized). Returns `true`
/// if the value was stored, `false` if a value had already been set.
///
/// Default: 32.
pub fn set_global_max_threads(count: usize) -> bool {
    MAX_THREADS.set(count).is_ok()
}

/// Set the process-wide maximum RAM budget, in bytes, shared by all `VmJs`
/// instances.
///
/// This only takes effect if called before the first `VmJs` is created
/// (i.e. before the process-global budget is initialized). Returns `true`
/// if the value was stored, `false` if a value had already been set.
///
/// Default: 768 MiB.
pub fn set_global_max_ram_bytes(bytes: usize) -> bool {
    MAX_RAM_BYTES.set(bytes).is_ok()
}

/// A process-wide (or custom) resource budget shared across `VmJs`
/// instances.
///
/// Most callers do not construct this directly; the process-wide budget is
/// available via [VmJsGlobal::global] and configured through
/// [set_global_max_threads] / [set_global_max_ram_bytes]. Constructing one
/// directly is useful for tests or to give a group of instances an isolated
/// budget.
pub struct VmJsGlobal {
    max_threads: usize,
    max_ram_bytes: usize,
    threads: Arc<tokio::sync::Semaphore>,
    ram_mib: Arc<tokio::sync::Semaphore>,
}

/// Permits held by a [crate::VmJs] for its lifetime. Dropping this releases
/// the thread and RAM reservations back to the budget.
pub struct GlobalPermits {
    _thread: tokio::sync::OwnedSemaphorePermit,
    _ram: tokio::sync::OwnedSemaphorePermit,
}

impl std::fmt::Debug for GlobalPermits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalPermits").finish_non_exhaustive()
    }
}

impl VmJsGlobal {
    /// Construct a new budget allowing `max_threads` concurrent instances
    /// and `max_ram_bytes` of total RAM. The RAM budget is clamped to a
    /// minimum of 1 MiB.
    pub fn new(max_threads: usize, max_ram_bytes: usize) -> Arc<Self> {
        let max_ram_bytes = max_ram_bytes.max(MIB);
        let max_ram_mib = max_ram_bytes / MIB;
        Arc::new(Self {
            max_threads,
            max_ram_bytes,
            threads: Arc::new(tokio::sync::Semaphore::new(
                max_threads.min(tokio::sync::Semaphore::MAX_PERMITS),
            )),
            ram_mib: Arc::new(tokio::sync::Semaphore::new(
                max_ram_mib.min(tokio::sync::Semaphore::MAX_PERMITS),
            )),
        })
    }

    /// Get the process-wide budget, initializing it on first use from the
    /// [set_global_max_threads] / [set_global_max_ram_bytes] overrides (or
    /// the defaults 32 threads / 768 MiB).
    pub fn global() -> Arc<Self> {
        static GLOBAL: OnceLock<Arc<VmJsGlobal>> = OnceLock::new();
        GLOBAL
            .get_or_init(|| {
                let threads = *MAX_THREADS.get().unwrap_or(&32);
                let ram = *MAX_RAM_BYTES.get().unwrap_or(&(768 * MIB));
                VmJsGlobal::new(threads, ram)
            })
            .clone()
    }

    /// The configured thread cap.
    pub fn max_threads(&self) -> usize {
        self.max_threads
    }

    /// The configured RAM budget, in bytes.
    pub fn max_ram_bytes(&self) -> usize {
        self.max_ram_bytes
    }

    /// Acquire a thread permit and a RAM permit for an instance requesting
    /// `max_mem_bytes` of per-instance quota.
    ///
    /// Waits up to `timeout` for both permits; on expiry returns a fatal
    /// error. A request larger than the whole budget fails immediately
    /// rather than waiting forever.
    pub async fn acquire(
        &self,
        max_mem_bytes: usize,
        timeout: Duration,
    ) -> JsResult<GlobalPermits> {
        // round the per-instance quota up to whole MiB
        let need_mib = max_mem_bytes.div_ceil(MIB).max(1);
        let max_ram_mib = self.max_ram_bytes / MIB;
        if need_mib > max_ram_mib {
            return Err(JsError::Fatal(std::io::Error::other(format!(
                "requested max_mem_bytes ({max_mem_bytes}) exceeds the \
                 global ram budget ({} bytes)",
                self.max_ram_bytes
            ))));
        }
        let need_mib = u32::try_from(need_mib).unwrap_or(u32::MAX);

        match tokio::time::timeout(timeout, self.acquire_inner(need_mib)).await
        {
            Ok(res) => res,
            Err(_) => Err(JsError::Fatal(std::io::Error::other(
                "timed out waiting for global javascript resource budget",
            ))),
        }
    }

    async fn acquire_inner(&self, need_mib: u32) -> JsResult<GlobalPermits> {
        // Acquire the thread permit before the RAM permit. A fixed order
        // avoids the classic hold-and-wait cycle: with independent
        // semaphores, two tasks each grabbing one permit via `try_join!`
        // could deadlock (A holds thread/waits RAM, B holds RAM/waits
        // thread). Ordering the waits makes a cycle impossible. If this
        // future is dropped while waiting (e.g. on timeout), any held
        // thread permit is released with it.
        let thread =
            self.threads.clone().acquire_owned().await.map_err(|_| {
                JsError::Fatal(std::io::Error::other(
                    "global thread semaphore closed",
                ))
            })?;
        let ram = self
            .ram_mib
            .clone()
            .acquire_many_owned(need_mib)
            .await
            .map_err(|_| {
                JsError::Fatal(std::io::Error::other(
                    "global ram semaphore closed",
                ))
            })?;
        Ok(GlobalPermits {
            _thread: thread,
            _ram: ram,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquire_enforces_thread_limit() {
        let g = VmJsGlobal::new(2, 1024 * MIB);

        let p1 = g.acquire(MIB, Duration::from_secs(1)).await.unwrap();
        let p2 = g.acquire(MIB, Duration::from_secs(1)).await.unwrap();

        // the third thread slot is taken; this must time out
        let err = g.acquire(MIB, Duration::from_millis(50)).await.unwrap_err();
        assert!(err.to_string().contains("timed out"), "{err:?}");

        // releasing one lets the next through
        drop(p1);
        let p3 = g.acquire(MIB, Duration::from_secs(1)).await.unwrap();
        drop(p2);
        drop(p3);
    }

    #[tokio::test]
    async fn acquire_enforces_ram_limit() {
        // 4 MiB of RAM, plenty of threads
        let g = VmJsGlobal::new(10, 4 * MIB);

        let p1 = g.acquire(3 * MIB, Duration::from_secs(1)).await.unwrap();

        // only 1 MiB left, a 2 MiB request must wait then time out
        let err = g
            .acquire(2 * MIB, Duration::from_millis(50))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("timed out"), "{err:?}");

        // after releasing, the full budget is available again
        drop(p1);
        let _p2 = g.acquire(4 * MIB, Duration::from_secs(1)).await.unwrap();
    }

    #[tokio::test]
    async fn acquire_rejects_oversized_request() {
        let g = VmJsGlobal::new(10, 4 * MIB);

        // bigger than the entire budget: fail immediately, don't hang
        let err = g
            .acquire(8 * MIB, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("exceeds"), "{err:?}");
    }

    #[tokio::test]
    async fn acquire_rounds_up_to_mib() {
        // 2 MiB budget; a 1-byte request rounds up to consume a whole MiB
        let g = VmJsGlobal::new(10, 2 * MIB);

        let p1 = g.acquire(1, Duration::from_secs(1)).await.unwrap();
        let p2 = g.acquire(1, Duration::from_secs(1)).await.unwrap();

        let err = g.acquire(1, Duration::from_millis(50)).await.unwrap_err();
        assert!(err.to_string().contains("timed out"), "{err:?}");

        drop(p1);
        drop(p2);
    }

    #[tokio::test]
    async fn permits_release_on_drop() {
        let g = VmJsGlobal::new(1, MIB);

        let p = g.acquire(MIB, Duration::from_secs(1)).await.unwrap();
        drop(p);

        // available again
        let _p = g.acquire(MIB, Duration::from_secs(1)).await.unwrap();
    }
}
