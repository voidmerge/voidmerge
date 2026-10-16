use super::*;

#[tokio::test]
async fn sanity() {
    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: "function bob(a) { return a + 1; }".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let s1 = std::time::Instant::now();

    let res = j
        .call("bob", 42, std::time::Duration::from_secs(10))
        .await
        .unwrap();

    println!("s1: {}s", s1.elapsed().as_secs_f64());

    assert_eq!(43, res);

    let s2 = std::time::Instant::now();

    let res = j
        .call("bob", 99, std::time::Duration::from_secs(10))
        .await
        .unwrap();

    println!("s2: {}s", s2.elapsed().as_secs_f64());

    assert_eq!(100, res);

    j.blocking_shutdown();
}

fn ext_cb() -> ExtensionCb {
    Arc::new(|| default_extensions::default_extensions(None))
}

async fn exec_ext_test(code: &str) {
    let j: VmJs<(), ()> = VmJs::new(VmJsConfig {
        code: code.into(),
        extension_cb: ext_cb(),
        ..Default::default()
    })
    .await
    .unwrap();

    j.call("test", (), std::time::Duration::from_secs(10))
        .await
        .unwrap();

    j.blocking_shutdown();
}

#[tokio::test]
async fn ext_encoding() {
    exec_ext_test(
        r#"function test() {
if (btoa("foo") !== "Zm9v") {
    throw new Error("btoa failure");
}
if (atob("Zm9v") !== "foo") {
    throw new Error("atob failure");
}
if (new TextDecoder().decode(new TextEncoder().encode("hello")) !== "hello") {
    throw new Error("Text Encoder / Decoder failure");
}
        }"#,
    )
    .await;
}

#[tokio::test]
async fn ext_url() {
    exec_ext_test(
        r#"function test() {
const expected = "http://127.0.0.1:123/path?a=1&b=2#test";

const t1 = new URL(expected);
const t2 = new URL("https://stub");
t2.protocol = "http://";
t2.hostname = "127.0.0.1";
t2.port = 123;
t2.pathname = "path";
t2.search = "a=1&b=2";
t2.hash = "test";

if (t1.toString() !== expected) {
  throw new Error(`URL t1 expected '${expected}', got: '${t1.toString()}'`);
}

if (t2.toString() !== expected) {
  throw new Error(`URL t2 expected '${expected}', got: '${t2.toString()}'`);
}
        }"#,
    )
    .await;
}

#[tokio::test]
async fn ext_timers() {
    exec_ext_test(
        r#"async function test() {
let timeout1Complete = false;
let timeout2Complete = false;

setTimeout(() => {
  timeout1Complete = true;
}, 50);

const id = setTimeout(() => {
  timeout2Complete = true;
}, 50);

await new Promise((res) => setTimeout(res, 10));

clearTimeout(id);

await new Promise((res) => setTimeout(res, 60));

if (timeout1Complete === false) {
  throw new Error('timeout1Complete was not true, setTimeout failed');
}

if (timeout2Complete === true) {
  throw new Error('timeout2Complete was true, clearTimeout failed');
}

let interval1Count = 0;
let interval2Count = 0;

const id1 = setInterval(() => {
  interval1Count += 1;
}, 10);

const id2 = setInterval(() => {
  interval2Count += 1;
}, 10);

await new Promise((res) => setTimeout(res, 15));

clearInterval(id2);

await new Promise((res) => setTimeout(res, 60));

clearInterval(id1);

if (interval1Count < 4) {
  throw new Error(`expected interval1Count > 3, got: ${interval1Count}`);
}

if (interval2Count > 3) {
  throw new Error(`expected interval2Count <= 3, got: ${interval2Count}`);
}
        }"#,
    )
    .await;
}

#[tokio::test]
async fn ext_crypto() {
    exec_ext_test(
        r#"async function test() {
const MSG = new TextEncoder().encode('hello world');
const MSG2 = new TextEncoder().encode('bad message');

// getRandomValues
const randNotExpected = new Uint8Array(16).toString();
const rand = crypto.getRandomValues(new Uint8Array(16));
if (rand === randNotExpected) {
  throw new Error(`expected random data, got: ${rand}`);
}

// sha256
const hash = Array.from(new Uint8Array(await crypto.subtle.digest(
  'SHA-256',
  MSG,
))).map(b => b.toString(16).padStart(2, '0')).join('');
const hashExpected = 'b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9';
if (hash !== hashExpected) {
  throw new Error(`sha256 expected '${hashExpected}', got: '${hash}'`);
}

// p256
const p256keys = await crypto.subtle.generateKey(
  { name: 'ECDSA', namedCurve: 'P-256' },
  true,
  ['sign', 'verify'],
);
const sig = await crypto.subtle.sign(
  { name: 'ECDSA', hash: { name: 'SHA-256' } },
  p256keys.privateKey,
  MSG,
);
const isValid = await crypto.subtle.verify(
  { name: 'ECDSA', hash: { name: 'SHA-256' } },
  p256keys.publicKey,
  sig,
  MSG,
);
if (!isValid) {
  throw new Error('invalid p256 signature');
}
const isValid2 = await crypto.subtle.verify(
  { name: 'ECDSA', hash: { name: 'SHA-256' } },
  p256keys.publicKey,
  sig,
  MSG2,
);
if (isValid2) {
  throw new Error('unexpected valid p256 signature');
}

// aes 256 gcm
const aesKey = await crypto.subtle.generateKey(
  { name: 'AES-GCM', length: 256 },
  true,
  ['encrypt', 'decrypt'],
);
const aesIv = crypto.getRandomValues(new Uint8Array(12));
const aesCipher = await crypto.subtle.encrypt(
  { name: 'AES-GCM', iv: aesIv },
  aesKey,
  MSG,
);
const aesPlain = await crypto.subtle.decrypt(
  { name: 'AES-GCM', iv: aesIv },
  aesKey,
  aesCipher,
);
const aesPlainStr = new TextDecoder().decode(new Uint8Array(aesPlain));
if (aesPlainStr !== 'hello world') {
  throw new Error(`expected: 'hello world', got: '${aesPlainStr}'`);
}
        }"#,
    )
    .await;
}

#[tokio::test]
async fn ext_console() {
    let logs: Arc<std::sync::Mutex<Vec<(String, String)>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let logs2 = logs.clone();

    let j: VmJs<(), ()> = VmJs::new(VmJsConfig {
        code: r#"function test() {
            console.log("hello", 42, true);
            console.debug("dbg");
            console.info("inf");
            console.warn("wrn");
            console.error("err");
            console.trace("trc");
            console.log({ a: 1 }, [1, 2]);
        }"#
        .into(),
        extension_cb: Arc::new(move || {
            let logs2 = logs2.clone();
            default_extensions::default_extensions(Some(Arc::new(
                move |level, message| {
                    logs2.lock().unwrap().push((level, message));
                },
            )))
        }),
        ..Default::default()
    })
    .await
    .unwrap();

    j.call("test", (), std::time::Duration::from_secs(10))
        .await
        .unwrap();

    j.blocking_shutdown();

    let logs = logs.lock().unwrap();
    assert_eq!(
        *logs,
        vec![
            ("log".to_string(), "hello 42 true".to_string()),
            ("debug".to_string(), "dbg".to_string()),
            ("info".to_string(), "inf".to_string()),
            ("warn".to_string(), "wrn".to_string()),
            ("error".to_string(), "err".to_string()),
            ("trace".to_string(), "trc".to_string()),
            ("log".to_string(), "[object Object] 1,2".to_string()),
        ]
    );
}

#[tokio::test]
async fn timeout() {
    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: "function bob(a) { while (true) {}; return a + 1; }".into(),
        ..Default::default()
    })
    .await
    .unwrap();

    let res = j
        .call("bob", 42, std::time::Duration::from_millis(10))
        .await
        .unwrap_err();

    // a CPU-bound loop never yields to the executor, so the tokio timer
    // can't fire; the monitor interrupt terminates it and we normalize the
    // result to a clear, fatal timeout
    assert!(res.to_string().contains("timed out"), "{res:?}");

    let res = j
        .call("bob", 42, std::time::Duration::from_millis(10))
        .await
        .unwrap_err();

    // second call we get shut down because the thread isn't even running
    assert!(res.to_string().contains("shut down"), "{res:?}");
}

/// Regression test for a timeout that the old monitor could not enforce:
/// the isolate is parked awaiting a slow async op, so no JS is executing
/// and the interrupt callback never fires. The event-loop-level tokio
/// timeout must bound this.
#[tokio::test]
async fn timeout_async_op() {
    #[deno_core::op2]
    async fn my_sleep() -> Result<(), deno_core::error::CoreError> {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        Ok(())
    }
    deno_core::extension!(my_sleep_ext, ops = [my_sleep]);

    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: r#"async function bob(a) {
            await Deno.core.ops.my_sleep();
            return a + 1;
        }"#
        .into(),
        extension_cb: Arc::new(|| vec![my_sleep_ext::init()]),
        ..Default::default()
    })
    .await
    .unwrap();

    let start = std::time::Instant::now();
    let res = j
        .call("bob", 42, std::time::Duration::from_millis(100))
        .await
        .unwrap_err();
    let elapsed = start.elapsed();

    assert!(res.to_string().contains("timed out"), "{res:?}");
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "elapsed: {elapsed:?}"
    );

    // a timeout is fatal, the isolate is gone
    let res = j
        .call("bob", 42, std::time::Duration::from_millis(100))
        .await
        .unwrap_err();
    assert!(res.to_string().contains("shut down"), "{res:?}");
}

/// Regression test for the old `setTimeout` escape hatch: a client could
/// `await` a timer for days despite a short timeout.
#[tokio::test]
async fn timeout_set_timeout() {
    let j: VmJs<(), ()> = VmJs::new(VmJsConfig {
        code: r#"async function test() {
            await new Promise((res) => setTimeout(res, 5000));
        }"#
        .into(),
        extension_cb: ext_cb(),
        ..Default::default()
    })
    .await
    .unwrap();

    let start = std::time::Instant::now();
    let res = j
        .call("test", (), std::time::Duration::from_millis(100))
        .await
        .unwrap_err();
    let elapsed = start.elapsed();

    assert!(res.to_string().contains("timed out"), "{res:?}");
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "elapsed: {elapsed:?}"
    );

    let res = j
        .call("test", (), std::time::Duration::from_millis(100))
        .await
        .unwrap_err();
    assert!(res.to_string().contains("shut down"), "{res:?}");
}

/// Guard against the event-loop timeout firing spuriously: a call that
/// completes comfortably within its deadline must succeed.
#[tokio::test]
async fn timeout_completes_before_deadline() {
    #[deno_core::op2]
    async fn my_quick_sleep() -> Result<(), deno_core::error::CoreError> {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        Ok(())
    }
    deno_core::extension!(my_quick_ext, ops = [my_quick_sleep]);

    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: r#"async function bob(a) {
            await Deno.core.ops.my_quick_sleep();
            return a + 1;
        }"#
        .into(),
        extension_cb: Arc::new(|| vec![my_quick_ext::init()]),
        ..Default::default()
    })
    .await
    .unwrap();

    let res = j
        .call("bob", 42, std::time::Duration::from_secs(5))
        .await
        .unwrap();

    assert_eq!(43, res);
}

#[tokio::test]
async fn array_buffer_mem_quota() {
    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: r#"function bob(a) {
            const mem = [];
            while (true) {
                mem.push(new Uint8Array(1024));
            };
            return a + 1;
        }"#
        .into(),
        max_mem_bytes: 12 * 1024 * 1024,
        ..Default::default()
    })
    .await
    .unwrap();

    let res = j
        .call("bob", 42, std::time::Duration::from_millis(10))
        .await
        .unwrap_err();

    // memory is enforced at allocation time, so the isolate is torn down
    // as soon as it tries to exceed its quota
    assert!(
        res.to_string().contains("array buffer memory quota"),
        "{res:?}"
    );

    // the isolate is gone, subsequent calls must fail
    let res = j
        .call("bob", 42, std::time::Duration::from_millis(10))
        .await
        .unwrap_err();

    assert!(res.to_string().contains("shut down"), "{res:?}");
}

/// Regression test for a single large allocation overshooting the quota.
/// Before enforcement moved into the allocator, a script could allocate
/// hundreds of MiB in one statement before the 100ms monitor thread
/// noticed.
#[tokio::test]
async fn array_buffer_single_alloc_rejected() {
    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: r#"function bob(a) {
            const one = new Uint8Array(256 * 1024 * 1024);
            one.fill(1);
            return a + 1;
        }"#
        .into(),
        max_mem_bytes: 16 * 1024 * 1024,
        ..Default::default()
    })
    .await
    .unwrap();

    let res = j
        .call("bob", 42, std::time::Duration::from_millis(500))
        .await
        .unwrap_err();

    // the 256 MiB allocation must be rejected up-front rather than being
    // allowed to balloon the process
    assert!(
        res.to_string().contains("array buffer memory quota"),
        "{res:?}"
    );
}

#[tokio::test]
async fn js_mem_quota() {
    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: r#"function bob(a) {
            const mem = [];
            while (true) {
                mem.push('a'.repeat(512));
            };
            return a + 1;
        }"#
        .into(),
        max_mem_bytes: 12 * 1024 * 1024,
        ..Default::default()
    })
    .await
    .unwrap();

    let res = j
        .call("bob", 42, std::time::Duration::from_millis(10))
        .await
        .unwrap_err();

    assert!(res.to_string().contains("terminated"), "{res:?}");
}

/// An injected global budget must cap the number of live instances. With a
/// one-thread budget, a second `VmJs` cannot be created until the first is
/// dropped.
#[tokio::test]
async fn global_budget_thread_cap() {
    let budget = VmJsGlobal::new(1, 64 * 1024 * 1024);

    let mk = || VmJsConfig {
        code: "function bob(a) { return a + 1; }".into(),
        max_mem_bytes: 16 * 1024 * 1024,
        global: Some(budget.clone()),
        global_acquire_timeout: std::time::Duration::from_millis(100),
        ..Default::default()
    };

    let a: VmJs<usize, usize> = VmJs::new(mk()).await.unwrap();

    // the single thread permit is held; this must time out
    let err = VmJs::<usize, usize>::new(mk()).await.err().unwrap();
    assert!(err.to_string().contains("timed out"), "{err:?}");

    // dropping the first releases the permit
    drop(a);
    let b: VmJs<usize, usize> = VmJs::new(mk()).await.unwrap();
    b.blocking_shutdown();
}

/// An injected global RAM budget must cap total per-instance quotas, even
/// when thread slots remain.
#[tokio::test]
async fn global_budget_ram_cap() {
    // 24 MiB of RAM, plenty of threads
    let budget = VmJsGlobal::new(10, 24 * 1024 * 1024);

    let mk = || VmJsConfig {
        code: "function bob(a) { return a + 1; }".into(),
        max_mem_bytes: 16 * 1024 * 1024,
        global: Some(budget.clone()),
        global_acquire_timeout: std::time::Duration::from_millis(100),
        ..Default::default()
    };

    let a: VmJs<usize, usize> = VmJs::new(mk()).await.unwrap();

    // only 8 MiB left, so a second 16 MiB instance cannot be created
    let err = VmJs::<usize, usize>::new(mk()).await.err().unwrap();
    assert!(err.to_string().contains("timed out"), "{err:?}");

    drop(a);
    let b: VmJs<usize, usize> = VmJs::new(mk()).await.unwrap();
    b.blocking_shutdown();
}

/// A per-instance quota larger than the whole budget fails fast instead of
/// waiting for permits that can never be satisfied.
#[tokio::test]
async fn global_budget_oversized_request() {
    let budget = VmJsGlobal::new(10, 4 * 1024 * 1024);

    let err = VmJs::<usize, usize>::new(VmJsConfig {
        code: "function bob(a) { return a + 1; }".into(),
        max_mem_bytes: 16 * 1024 * 1024,
        global: Some(budget),
        global_acquire_timeout: std::time::Duration::from_millis(100),
        ..Default::default()
    })
    .await
    .err()
    .unwrap();

    assert!(err.to_string().contains("exceeds"), "{err:?}");
}

#[tokio::test]
async fn extension_sanity() {
    #[deno_core::op2]
    #[string]
    async fn my_fn(
        #[string] param1: String,
    ) -> Result<String, deno_core::error::CoreError> {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        Ok(format!("Got: {param1}"))
    }
    deno_core::extension!(my_ext, ops = [my_fn]);

    let j: VmJs<String, String> = VmJs::new(VmJsConfig {
        code: "function bob(a) { return Deno.core.ops.my_fn(a); }".into(),
        extension_cb: Arc::new(|| vec![my_ext::init()]),
        ..Default::default()
    })
    .await
    .unwrap();

    let res = j
        .call(
            "bob",
            "test".to_string(),
            std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();

    assert_eq!("Got: test".to_string(), res);
}
