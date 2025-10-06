pub use crate::integration::setup::Test;

#[tokio::test(flavor = "multi_thread")]
async fn obj_simple() {
    let test = Test::new("obj-simple").await;

    #[derive(Debug, serde::Deserialize)]
    struct R1 {
        meta: String,
    }

    let r1: R1 = test.test_fn_req(serde_json::json!({
        "do": "put",
        "k": "bob",
        "v": "hello",
    })).await;

    println!("put result: {r1:?}");

    #[derive(Debug, serde::Deserialize)]
    struct R2 {
        list: Vec<String>,
    }

    let r2: R2 = test.test_fn_req(serde_json::json!({
        "do": "list",
        "k": "b",
    })).await;

    println!("list result: {r2:?}");

    assert_eq!(vec![r1.meta.clone()], r2.list);

    #[derive(Debug, serde::Deserialize)]
    struct R3 {
        val: String,
    }

    let r3: R3 = test.test_fn_req(serde_json::json!({
        "do": "get",
        "k": r1.meta,
    })).await;

    println!("get result: {r3:?}");

    assert_eq!("hello", r3.val);
}
