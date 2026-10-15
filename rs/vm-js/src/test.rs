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

async fn exec_ext_test(code: &str) {
    deno_core::extension!(
        vm_bootstrap,
        deps = [deno_webidl, deno_web, deno_crypto],
        esm_entry_point = "ext:vm_bootstrap/vm_bootstrap.js",
        esm = ["vm_bootstrap.js"],
    );

    let extension_cb: ExtensionCb = Arc::new(|| {
        vec![
            deno_webidl::deno_webidl::init(),
            deno_web::deno_web::init(
                Arc::new(deno_web::BlobStore::default()),
                None,
                true,
                deno_web::InMemoryBroadcastChannel::default(),
            ),
            deno_crypto::deno_crypto::init(None),
            vm_bootstrap::init(),
        ]
    });

    let j: VmJs<(), ()> = VmJs::new(VmJsConfig {
        code: code.into(),
        extension_cb,
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

    // for now we get a terminated... would be nice to send up the timeout
    assert!(res.to_string().contains("terminated"), "{res:?}");

    let res = j
        .call("bob", 42, std::time::Duration::from_millis(10))
        .await
        .unwrap_err();

    // second call we get shut down because the thread isn't even running
    assert!(res.to_string().contains("shut down"), "{res:?}");
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

    assert!(res.to_string().contains("terminated"), "{res:?}");
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
