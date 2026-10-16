# vm-js: Memory & Timing Guard Evaluation — TODO

Date: 2026-10-15
Scope: `rs/vm-js` (the `vm-js` crate)
Status: evaluation complete; fixes not yet implemented

## 0. Why this document exists

`vm-js` is intended to run **downstream client JavaScript on a shared
server alongside other clients**. That means the code it runs is at best
semi-trusted and at worst actively hostile. The crate tries to contain each
`VmJs` instance with a memory quota and a per-call timeout. This document
evaluates those guards, records what was verified empirically, and lists
prioritized recommendations.

It is written to be readable by someone with **no prior context on the
crate**. Section 1 explains the architecture. Section 2 lists findings.
Section 3 is the prioritized TODO.

All line references are against the tree as of the date above.

## 1. Background: how the guards work today

`vm-js` wraps `deno_core`/V8. Each `VmJs` instance owns a dedicated OS
thread running a single-threaded `tokio` runtime, which in turn owns one V8
isolate. Calls are serialized over an `mpsc` channel (`lib.rs:118`,
`js_thread.rs:115-166`).

There are four pieces to the resource guarding:

1. **V8 heap limit** — the isolate is created with
   `heap_limits(0, config.max_mem_bytes)` (`js_thread.rs:75`). This caps the
   V8-managed heap only.
2. **ArrayBuffer tracking allocator** — V8 does not count `ArrayBuffer`
   memory (e.g. `Uint8Array`) against its heap. The crate installs a custom
   V8 `RustAllocator` (`alloc.rs`) that increments/decrements an
   `Arc<AtomicUsize>` on allocate/free. This counter is `ab_bytes`.
3. **Global monitor thread** — a single process-wide thread sleeps 100 ms,
   then iterates every registered isolate and calls
   `IsolateHandle::request_interrupt(mem_interrupt_cb, ...)`
   (`monitor.rs:100-123`). The interrupt callback (`monitor.rs:125-171`),
   which runs on the isolate's own thread at a V8 safepoint, checks three
   things and calls `terminate_execution()` if any fail:
   - the cancellation token,
   - the active call's deadline (`timeout_at`),
   - `total_heap_size() + ab_bytes > max_mem_bytes`.
4. **Near-heap-limit fallback** — `add_near_heap_limit_callback`
   (`js_thread.rs:87-97`) cancels and terminates if the pure V8 heap grows
   faster than the monitor's 100 ms cadence, returning `cur * 2` to give V8
   room to unwind.

Timeouts are set/cleared around each call via `set_timeout`/`clear_timeout`
(`monitor.rs:51-65`, called from `js_thread.rs:130,135,206,214`). A timeout
is treated as **fatal**: the isolate is cancelled and never reused, because
a terminated execution can leave "zombie promises" that would corrupt later
calls.

Default quota: `max_mem_bytes = 32 MiB` (`lib.rs:78`). Setup code gets a
hardcoded 10 s timeout (`js_thread.rs:6`).

## 2. Findings

Severity legend: **CRITICAL** = blocks safe multi-tenant use;
**HIGH** = serious; **MEDIUM** = worth fixing; **LOW** = polish.

### 2.1 [CRITICAL] Memory quota is not enforced at allocation time

`ab_alloc_zeroed` / `ab_alloc_uninit` (`alloc.rs:16-50`) only *count* bytes.
They never compare against `max_mem_bytes`; in fact the limit is never even
passed to `new_tracking_allocator` (`alloc.rs:84-99`). Enforcement is purely
asynchronous, via the 100 ms monitor poll. Therefore a **single JS
statement** can allocate an arbitrarily large `ArrayBuffer` before the
monitor reacts.

**Verified empirically.** With `max_mem_bytes = 16 MiB`, running:

```js
const one = new Uint8Array(256 * 1024 * 1024);
one.fill(1);
while (true) {}
```

drove committed RSS up by **~280 MiB (~17× the limit)** before the call was
terminated. A script allocating `new Uint8Array(8 * 1024**3)` is an
OOM-killer / process-abort vector rather than a graceful termination.

Note: a bare `new Uint8Array(N)` only reserves address space lazily; the
overshoot is realized when the pages are touched (`.fill(...)`, reads,
crypto, etc.). Real scripts touch their buffers.

### 2.2 [CRITICAL] Timeouts do not bound async awaits

**Status: FIXED 2026-10-15 (see 3.2).** The description below documents the
pre-fix behavior.

The timeout is only evaluated inside the V8 interrupt callback, which only
runs while the isolate is **executing JS**. While JS is parked awaiting a
promise, no JS runs, so the callback never fires. The 100 ms monitor thread
calls `request_interrupt`, but that does not wake a parked event loop.

**Verified empirically** with a 100 ms timeout:

| JS under test | Observed wall time |
| --- | --- |
| `await` an async op that sleeps 2000 ms | **2.0 s** |
| `await new Promise(r => setTimeout(r, 5000))` | **5.0 s** |

Because `setTimeout` accepts delays up to ~2^31 ms (~24.8 days), a client
can pin a worker thread for days despite a 1 s timeout. This is a
straightforward shared-server DoS.

Mitigating detail: a promise that never resolves *while the event loop is
otherwise idle* **is** caught — `deno_core` returns `PendingPromiseResolution`
when it cannot make progress. But any live timer or pending op (including a
`setInterval` heartbeat) defeats the timeout, as does any slow async op.

### 2.3 [HIGH] No global budget across instances

Every `VmJs` independently gets up to `max_mem_bytes` (`lib.rs:68,78`).
Nothing bounds the total across clients, and there is no thread cap. The
older implementation in `rs/voidmerge/src/js.rs` did have both:
`MAX_RAM`/`ram_mib_limit` and `MAX_THREADS`/`thread_limit` semaphores
(`js.rs:123-143, 276-314`). The rewrite dropped these.

### 2.4 [HIGH] `max_mem_bytes` undercounts real memory

The check is `total_heap_size() + ab_bytes`. It omits:

- Rust allocations inside ops (`deno_web` blobs, message ports, buffers),
- V8 external memory / external strings,
- the 32-deep `mpsc` queue holding `Input`/`Output` values (`lib.rs:118`),
- `serde_v8` serialization buffers,
- per-thread stack (MBs each) and V8's per-isolate baseline.

Additionally, the near-heap-limit fallback returns `cur * 2`
(`js_thread.rs:96`), allowing a transient heap overshoot up to ~2× the limit
between callbacks. So `max_mem_bytes` is a heap+AB budget, not a process
budget; real per-client footprint is meaningfully higher.

### 2.5 [MEDIUM] `Drop` detaches the thread

`VmJs::drop` cancels but does not join (`lib.rs:100-108`; the join is
commented out). `blocking_shutdown` exists (`lib.rs:139-144`) but callers
must opt in. With no thread cap (see 2.3), create/drop churn can accumulate
OS threads and V8 isolates. A thread stuck in an uninterruptible native op
(2.7) is leaked permanently, with no handle left to reclaim it.

### 2.6 [MEDIUM] Monitor callback locks a global mutex from an interrupt handler

**Status: FIXED 2026-10-15 (see 3.7).** The description below documents the
pre-fix behavior.

`mem_interrupt_cb` calls `mon_map()` (`monitor.rs:136`), taking a global
`Mutex<HashMap<...>>`. The module's own comment (`monitor.rs:83-85`) warns
against calling `mon_map()` in a JS execution context, yet the interrupt
handler is exactly that. It is currently deadlock-free only because no code
holds that lock across a V8 call; this is fragile and should be made
explicit/safe.

### 2.7 [MEDIUM] Blocking native code is uninterruptible

A synchronous op that blocks the isolate thread (`std::thread::sleep`, a
blocking lock, heavy synchronous crypto) will not service the interrupt
until it returns. Extensions are host-controlled, so this is partly
self-inflicted, but it means neither the timeout nor the memory guard can
preempt native code.

### 2.8 [LOW] Fixed 100 ms polling is coarse and wasteful

Timeouts can be exceeded by up to ~100 ms plus interrupt-delivery latency.
Meanwhile the monitor requests an interrupt on **every** isolate **every**
100 ms even when idle and within quota — an O(clients) cross-thread
interrupt storm with no benefit.

### 2.9 [LOW] Timeout result is misclassified

**Status: FIXED 2026-10-15 (partial 3.9, see 3.2).** Timeouts now surface as
a fatal `"javascript execution timed out"` on both the parked-await and
CPU-bound paths.

A timeout surfaces as `NonFatal("execution terminated")` (the monitor sets
`cancel` then terminates at `monitor.rs:157-158`; the JS error is mapped to
`NonFatal` at `js_thread.rs:219`), even though the isolate has been
cancelled. Callers see a non-fatal error, then a fatal "shut down" on the
next call.

### 2.10 [LOW] Misc

- ~~`Layout` alignment hardcoded to 8 in `alloc.rs:14`; the comment
  acknowledges this. Confirm it meets V8's requirement.~~ Reconfirmed
  2026-10-15: V8's `ArrayBuffer::Allocator` API imposes no alignment
  requirement (`v8-array-buffer.h`), so the constant was made a documented,
  conservative 16 bytes to match the platform allocator (see 3.9).
- ~~`Instant::now() + timeout` (`monitor.rs:54`) panics on overflow for
  pathological durations.~~ Fixed 2026-10-15: `set_timeout` now uses
  `checked_add` and treats overflow as "never" (see 3.7).
- `ab_bytes` uses `Ordering::Relaxed` (fine for approximate accounting).

## 3. Prioritized TODO

Ordered by risk. Items 3.1 and 3.2 are the blocking issues for untrusted
multi-tenant use; 3.4 is the durable backstop.

### 3.1 [CRITICAL] Enforce the ArrayBuffer budget inside the allocator

**Status: IMPLEMENTED 2026-10-15.**

Goal: no single allocation (or burst) can exceed the quota, even
momentarily.

Approach:

- Introduce a shared budget object, e.g.
  `Arc<AllocBudget { used: AtomicUsize, limit: usize }>` (or extend the
  existing `Arc<AtomicUsize>` into a struct holding both).
- Change `new_tracking_allocator` (`alloc.rs:84`) to take `max_mem_bytes`
  (and a small headroom for the isolate baseline / V8 heap interplay).
- In `ab_alloc_zeroed`/`ab_alloc_uninit`, atomically reserve before
  allocating (e.g. a `fetch_update` CAS loop, or
  `fetch_add` then check-and-rollback). On failure, return `null_mut()`.
  V8 turns a null ArrayBuffer allocation into a JS-visible failure rather
  than an OOM abort.
- On `ab_free`, release the reservation.
- Decide the interaction with the V8 heap limit: the allocator's limit
  should probably be `max_mem_bytes` minus a reserve for the V8 heap, or
  share a single combined budget.
- Keep the monitor check as a secondary net for heap growth.

Tests to add:

- Single huge allocation fails cleanly under a small quota (regression for
  the 16 MiB → 280 MiB overshoot).
- Repeated small allocations terminate near the quota.
- Legitimate workloads near the quota still succeed.

**Implementation notes (2026-10-15):**

- `alloc.rs`: replaced the raw `Arc<AtomicUsize>` handle with a public
  `AllocBudget { used, limit, over }`. `new_tracking_allocator(limit)` now
  takes the quota. `ab_alloc_zeroed`/`ab_alloc_uninit` call
  `AllocBudget::reserve(len)` (an atomic `fetch_update` CAS) before touching
  the system allocator, and `ab_free` calls `release`. A rejected
  reservation latches `over` and the callback returns `null_mut()`; V8
  surfaces this as `RangeError: Array buffer allocation failed` (verified —
  it does **not** abort the process).
- `js_thread.rs`: the allocator quota is `config.max_mem_bytes`. After each
  call, if `ab_budget.is_over()`, the isolate is treated as fatal and the
  thread exits with `"javascript exceeded its array buffer memory quota"`,
  even if the script caught the error and returned normally.
- `monitor.rs`: stores `Arc<AllocBudget>`; `mem_interrupt_cb` now checks
  `is_over()` as a backstop and uses `used()` for the combined heap+AB
  check.
- Interaction with the V8 heap limit (deferred): the allocator quota is the
  full `max_mem_bytes`, so AB alone cannot exceed it, but the combined
  heap+AB total can still transiently reach ~2× before the monitor samples.
  This is bounded and non-catastrophic; tightening it (e.g. a shared
  combined budget or heap-aware reserve) remains open. See also 3.5.
- Tests added: `alloc::tests::{reserve_enforces_limit,
  reserve_rejects_single_oversized_alloc, reserve_handles_overflow}` and
  `test::array_buffer_single_alloc_rejected`; `test::array_buffer_mem_quota`
  updated to expect the fatal quota error. All 13 tests pass.

### 3.2 [CRITICAL] Bound await time at the event-loop level

**Status: IMPLEMENTED 2026-10-15.** Approach #2 was chosen.

Goal: a call cannot exceed its timeout merely because it is awaiting a
promise/timer/op.

Approach options (in rough order of preference):

1. **Drive the event loop with a deadline.** Rather than only
   `with_event_loop_promise(...)` (`js_thread.rs:210-212`), poll the event
   loop yourself and check `Instant::now()` against the deadline between
   polls (deno_core exposes a lower-level `poll_event_loop`). On elapse,
   cancel + `terminate_execution()`.
2. **Race a timer against the call.** Wrap the event-loop future in a
   `tokio::time::timeout` on the JS thread's runtime; on elapse, cancel +
   terminate. Because a timeout is already fatal (isolate is discarded),
   abandoning the in-flight future is acceptable — but confirm deno_core
   tolerates dropping a runtime with pending ops, or prefer option 1.
3. **Expose a wake handle.** Give the monitor a way to wake the parked
   event loop so the interrupt can be delivered promptly.

Whichever is chosen, be explicit that **native code cannot be preempted**;
pair it with 3.4 for hard guarantees.

Tests to add:

- `await` a 2 s async op with a 100 ms timeout → must return in ~100 ms
  (regression for the observed 2.0 s).
- `await new Promise(r => setTimeout(r, 5000))` with a 100 ms timeout →
  must return in ~100 ms (regression for the observed 5.0 s).
- Existing CPU-bound `while(true)` timeout still works.

**Implementation notes (2026-10-15):**

- Chose **approach #2** for simplicity: `exec_call` now wraps
  `with_event_loop_promise(...)` in `tokio::time::timeout(timeout, ...)`
  (`js_thread.rs`). When JS parks on an `await`, the event-loop future
  returns `Poll::Pending`, the JS-thread task yields to the executor, and
  the tokio timer fires at the deadline (typically ~1 ms granularity).
- The monitor interrupt path is retained and remains the enforcement for
  **CPU-bound** code, which never yields to the executor, so the tokio
  timer cannot fire. The two mechanisms are complementary: tokio bounds
  parked awaits, the monitor bounds executing JS.
- Drop-safety was confirmed by inspection of `deno_core`: on timeout the
  in-flight event-loop future is dropped (no JS is executing at that
  instant, because `Timeout::poll` returns `Elapsed` without re-polling),
  then the `JsRuntime` is dropped. `InnerIsolateState::cleanup`
  (`deno_core-0.412.0/runtime/jsruntime.rs:244`) calls
  `pending_ops.shutdown()`, which aborts the op-driver task and clears the
  pending-op queue, so dropping with pending timers/ops is supported.
- **Partial 3.9:** a timeout now surfaces as a distinct, fatal
  `"javascript execution timed out"` error on both paths. The monitor
  latches a `timed_out` flag (`monitor.rs`) when it enforces a timeout, and
  `exec_call` checks it after the event loop returns, so the CPU-bound path
  no longer reports the confusing non-fatal `"execution terminated"`.
  Memory-termination paths are unaffected (they do not set the flag).
- Tests added: `test::timeout_async_op`, `test::timeout_set_timeout`,
  `test::timeout_completes_before_deadline`; `test::timeout` updated to
  expect `"timed out"`. The `deno_web` extension setup used by the
  extension tests was factored into a `test::ext_cb()` helper. All 16 tests
  pass.

### 3.3 [HIGH] Add a global budget and thread cap

**Status: IMPLEMENTED 2026-10-15** (semaphore approach; worker pool
deferred as out of scope).

Goal: N clients cannot collectively exhaust the host, and the number of
isolates is bounded.

Approach:

- Reintroduce a global RAM budget and thread limit as process-wide
  semaphores, as the old `rs/voidmerge/src/js.rs` did
  (`MAX_RAM`/`ram_mib_limit`, `MAX_THREADS`/`thread_limit`,
  `js.rs:123-143, 276-314`).
- Acquire a permit of `max_mem_bytes` (rounded up) and a thread permit when
  creating a `VmJs`; hold them for the instance's lifetime.
- Better: move to a **worker pool** and reuse isolates across requests
  instead of one OS thread per client. This bounds threads and amortizes
  V8's per-isolate baseline. Note isolate reuse requires resetting state
  between tenants, so weigh it against the current fatal-on-timeout
  teardown policy.
- Expose the limits via config (as the old `js_global_set_max_thread` /
  `js_global_set_max_ram` did).

**Implementation notes (2026-10-15):**

- New `global.rs` defines `VmJsGlobal`, a budget holding two
  `tokio::sync::Semaphore`s: one counting threads and one counting RAM in
  MiB. RAM is accounted in whole MiB because `Semaphore` takes a `u32`
  permit count (byte permits would overflow above 4 GiB). `VmJsGlobal::new`
  and `VmJsGlobal::global()` (the process-wide, lazily-initialized default
  of 32 threads / 768 MiB) construct one.
- Process-wide setters `set_global_max_threads` / `set_global_max_ram_bytes`
  are exposed at the crate root (the `OnceLock` equivalents of the old
  `js_global_set_max_thread` / `js_global_set_max_ram`); they must be called
  before the first `VmJs` is created.
- `VmJsConfig` gained `global: Option<Arc<VmJsGlobal>>` (defaults to the
  process-wide budget; an isolated budget can be injected for tests or
  per-tenant pools) and `global_acquire_timeout` (default 30 s).
- `VmJs::new` acquires a thread permit and a RAM permit (its `max_mem_bytes`
  rounded up to MiB) before spawning the isolate thread, holds them in a
  `GlobalPermits` field for the instance's lifetime, and releases them on
  drop. Acquisition is bounded by `global_acquire_timeout`; a request larger
  than the whole budget fails immediately with a fatal error rather than
  waiting forever.
- The two permits are acquired in a **fixed order** (thread, then RAM)
  rather than via `try_join!`: with independent semaphores, two tasks each
  grabbing one permit concurrently could deadlock (A holds thread/waits RAM,
  B holds RAM/waits thread). Fixed ordering makes a cycle impossible.
- Known limitation (see 3.6): `Drop` cancels but does not join the isolate
  thread, so a permit is released slightly before the OS thread actually
  exits; the cap can be transiently exceeded during teardown. Joining on
  drop remains the durable fix.
- Tests added: `global::tests::{acquire_enforces_thread_limit,
  acquire_enforces_ram_limit, acquire_rejects_oversized_request,
  acquire_rounds_up_to_mib, permits_release_on_drop}` (injected budgets);
  `test::{global_budget_thread_cap, global_budget_ram_cap,
  global_budget_oversized_request}` (wiring via `VmJs::new`); and the
  integration test `tests/global_budget.rs::process_global_thread_cap`
  (own process, so it can safely exercise the process-global setters).
  All 28 unit tests and the integration test pass.

### 3.4 [HIGH] Add a hard backstop via process isolation

Goal: a real, unbypassable ceiling for untrusted code.

- Run each isolate, or a small group of worker isolates, in a child
  process under a cgroup (or `rlimit`) memory cap plus a wall/CPU limit.
- On breach, `SIGKILL` the process. This survives OOM attempts, blocking
  native code (2.7), and any in-process accounting bug.
- In-process guards (3.1, 3.2) remain valuable for cheap, graceful
  enforcement; the cgroup is the guarantee of last resort.

### 3.5 [MEDIUM] Account for non-heap memory, or document the gap

- Fold V8 external memory (`adjust_amount_of_external_allocated_memory`),
  op-side Rust allocations, and serialization buffers into the budget where
  feasible.
- At minimum, document clearly that `max_mem_bytes` covers V8 heap +
  ArrayBuffers only, and size it accordingly (including per-thread stack and
  isolate baseline).

### 3.6 [MEDIUM] Join/reap threads on drop

- Either join on drop (possibly via `spawn_blocking`, as the old
  `JsThread::drop` did at `js.rs:826-859`) or route all instances through
  the pool from 3.3 so lifetimes are centrally managed.
- Ensure a thread stuck in native code is eventually killed (see 3.4)
  rather than leaked.

### 3.7 [MEDIUM] Make the monitor callback lock-free / self-contained

**Status: IMPLEMENTED 2026-10-15.**

- Avoid taking the global `mon_map` mutex inside `mem_interrupt_cb`
  (`monitor.rs:136`). Pass the monitor state to the callback directly (the
  `data` pointer already carries the `uniq`; consider carrying an
  `Arc<Monitor>` or a raw pointer to it), or use an atomic snapshot.
- Remove or correct the stale warning at `monitor.rs:83-85`.

**Implementation notes (2026-10-15):**

- `mon_thread` now passes `Arc::as_ptr(&mon) as *mut c_void` as the interrupt
  data instead of a `uniq`. `mem_interrupt_cb` reconstructs `&Monitor`
  directly from the pointer; it no longer calls `mon_map()` and takes no
  global lock.
- `Monitor.timeout_at` changed from `Mutex<Option<Instant>>` to an
  `AtomicU64` holding the deadline as nanoseconds since a per-monitor
  `start: Instant` (`0` = no active timeout). `set_timeout`/`clear_timeout`
  now store atomically and the callback reads via a lock-free `is_timed_out`
  helper. The callback is now fully lock-free.
- This also fixes the `Instant::now() + timeout` overflow (2.10): `set_timeout`
  uses `checked_add` and stores the `u64::MAX` sentinel ("never") on overflow.
- **Pointer-lifetime safety** (documented in a `// SAFETY:` comment): the
  data pointer is only dereferenced by the callback, which runs solely on the
  isolate thread while it executes javascript. The monitor map holds the
  `Arc<Monitor>` for the entire lifetime of that thread's call loop, so the
  pointer is valid whenever a callback can run. After the guard removes the
  map entry the thread no longer executes JS, and `request_interrupt` on a
  destroyed isolate is a no-op whose queued interrupts are discarded with the
  isolate.
- The `mon_map` doc comment was corrected: it is now only touched outside JS
  execution (registration/teardown, timeout set/clear, `timed_out`,
  `mark_timed_out`, and the monitor thread).
- Tests added: `monitor::tests::{nanos_since_is_monotonic_and_saturating,
  is_timed_out_handles_none_and_deadlines}`. The existing `timeout`,
  `js_mem_quota`, and `array_buffer_mem_quota` tests still exercise the
  callback end-to-end. All 18 tests pass.
- A direct deadlock regression test is not practical: no code path currently
  holds `mon_map` across a V8 call, so the failure isn't reproducible without
  inventing such a path; the fix is structural.

### 3.8 [LOW] Make the monitor deadline-driven and demand-based

- Track the earliest pending deadline and an atomic "over budget" flag set
  by the allocator (3.1).
- Sleep until the nearest event rather than a fixed 100 ms, and only
  `request_interrupt` isolates whose deadline has passed or whose over-budget
  flag is set. Eliminates the idle interrupt storm (2.8) and tightens
  timeout precision.

### 3.9 [LOW] Fix timeout error classification and misc

**Status: DONE 2026-10-15.**

- ~~Report timeouts distinctly and consistently; don't surface a cancelled
  isolate as `NonFatal` (2.9).~~ Done: both timeout paths now return a fatal
  `"javascript execution timed out"` (see 3.2 implementation notes).
- ~~Guard `Instant::now() + timeout` against overflow (`monitor.rs:54`).~~
  Done: `set_timeout` uses `checked_add` with a "never" sentinel (see 3.7).
- ~~Reconfirm the allocator alignment constant (`alloc.rs:14`).~~ Done:
  verified that V8's `ArrayBuffer::Allocator` documents no alignment
  requirement (vendored `v8-147.0.0/v8/include/v8-array-buffer.h`), and
  changed `ALIGN` from 8 to a documented 16 to match the platform allocator
  (`malloc`/`max_align_t`) that V8's default allocator uses. Added a
  compile-time power-of-two assertion and the
  `alloc::tests::allocator_returns_aligned_zeroed_pointers` test, which
  asserts returned pointers are `ALIGN`-aligned and that `Allocate` zeroes.

## 4. How the empirical findings were produced

Temporary tests were added to `src/test.rs` and then reverted; the tree is
clean. To reproduce:

- **Memory overshoot:** `max_mem_bytes = 16 MiB`; JS allocates a
  256 MiB `Uint8Array`, calls `.fill(1)`, then spins; sample
  `/proc/self/statm` RSS from another thread. Observed peak +280 MiB.
- **Async-op timeout:** a test-only `deno_core::op2` async op that sleeps
  2000 ms; call with a 100 ms timeout; measure elapsed. Observed 2.0 s.
- **Long-timer timeout:** `deno_web` timers extension; JS returns
  `new Promise(r => setTimeout(r, 5000))`; call with a 100 ms timeout.
  Observed 5.0 s.
- **Live event loop + timeout works:** `setInterval(..., 5)` plus a
  never-resolving promise with a 100 ms timeout terminated at ~105 ms,
  confirming the interrupt path does work when JS is actively executing.

## 5. Suggested implementation order

1. ~~3.1 (allocator budget)~~ — **done 2026-10-15**.
2. ~~3.2 (await-time bounding)~~ — **done 2026-10-15**.
3. ~~3.7 (lock-free monitor callback)~~ — **done 2026-10-15**.
4. ~~3.9 (timeout classification + misc)~~ — **done 2026-10-15**.
5. ~~3.3 (global budget + thread cap)~~ — **done 2026-10-15**.
6. 3.4 (process isolation backstop).
7. 3.5, 3.6, and 3.8 as follow-ups.

Add the regression tests from 3.1 and 3.2 alongside the fixes so the two
CRITICAL gaps cannot silently reappear.
