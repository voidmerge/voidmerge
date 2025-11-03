pub use crate::integration::setup::Test;

#[tokio::test(flavor = "multi_thread")]
async fn cron_simple() {
    let test = Test::new("cron-simple").await;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let r1: f64 = test.test_fn_req(serde_json::json!({})).await.unwrap();

    eprintln!("start cron exec count: {r1:?}");

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let r2: f64 = test.test_fn_req(serde_json::json!({})).await.unwrap();

    eprintln!("cron exec count after 200ms: {r2:?}");

    assert!(r2 > r1);
}
