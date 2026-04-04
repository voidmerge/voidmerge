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
        }"#.into(),
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
        }"#.into(),
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
