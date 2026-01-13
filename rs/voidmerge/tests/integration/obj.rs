pub use crate::integration::setup::Test;

#[tokio::test(flavor = "multi_thread")]
async fn obj_simple() {
    let test = Test::new("obj-simple").await;

    #[derive(Debug, serde::Deserialize)]
    struct R1 {
        meta: String,
    }

    assert!(
        test.test_fn_req::<R1>(serde_json::json!({
            "do": "put",
            "k": "bob",
            "v": "goodbye",
        }))
        .await
        .is_err()
    );

    let r1: R1 = test
        .test_fn_req(serde_json::json!({
            "do": "put",
            "k": "alice",
            "v": "hello",
        }))
        .await
        .unwrap();

    println!("put result (orig alice): {r1:?}");

    let orig_alice = r1.meta;

    let r1: R1 = test
        .test_fn_req(serde_json::json!({
            "do": "put",
            "k": "alice",
            "v": "hello",
        }))
        .await
        .unwrap();
    println!("put result (second alice): {r1:?}");

    #[derive(Debug, serde::Deserialize)]
    struct R0 {}

    test.test_fn_req::<R0>(serde_json::json!({
        "do": "rm",
        "k": orig_alice,
    }))
    .await
    .unwrap();

    #[derive(Debug, serde::Deserialize)]
    struct R2 {
        list: Vec<String>,
    }

    let r2: R2 = test
        .test_fn_req(serde_json::json!({
            "do": "list",
            "k": "a",
        }))
        .await
        .unwrap();

    println!("list result: {r2:?}");
    assert_eq!(0, r2.list.len());

    let r1: R1 = test
        .test_fn_req(serde_json::json!({
            "do": "put",
            "k": "bob",
            "v": "hello",
        }))
        .await
        .unwrap();

    println!("put result: {r1:?}");

    let r2: R2 = test
        .test_fn_req(serde_json::json!({
            "do": "list",
            "k": "b",
        }))
        .await
        .unwrap();

    println!("list result: {r2:?}");

    assert_eq!(vec![r1.meta.clone()], r2.list);

    #[derive(Debug, serde::Deserialize)]
    struct R3 {
        val: String,
    }

    let r3: R3 = test
        .test_fn_req(serde_json::json!({
            "do": "get",
            "k": r1.meta,
        }))
        .await
        .unwrap();

    println!("get result: {r3:?}");

    assert_eq!("hello", r3.val);
}
