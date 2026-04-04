use super::*;

#[tokio::test]
async fn sanity() {
    let j: VmJs<usize, usize> = VmJs::new(VmJsConfig {
        code: "function bob(a) { return a + 1; }".into(),
        ..Default::default()
    }).await.unwrap();
    let res = j.call("bob", 42).await.unwrap();
    assert_eq!(43, res);
}
