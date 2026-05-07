use super::*;

async fn exec(test_code: &str) {
    let rth = RuntimeHandle::default();
    let obj = obj::obj_file::ObjFile::create(None).await.unwrap();
    rth.set_obj(obj);

    let setup = JsSetup {
        runtime: rth.runtime(),
        ctx: "test".into(),
        env: Arc::new(serde_json::Value::Null),
        code: format!(
            r#"async function vm(req) {{
                const res = await test();

                if (res !== "TestPass") {{
                    throw new Error("Test Did Not Complete");
                }}

                return {{ type: 'fnResOk' }};
            }}

            async function test() {{

                {test_code}

                return "TestPass";
            }}"#
        )
        .into(),
        timeout: JsSetup::DEF_TIMEOUT,
        heap_size: JsSetup::DEF_HEAP_SIZE,
    };

    let req = JsRequest::FnReq {
        method: "GET".into(),
        path: "".into(),
        body: None,
        headers: Default::default(),
    };

    let js = JsExecDefault::create();

    let res = js.exec(setup, req).await.unwrap();

    match res {
        crate::js::JsResponse::FnResOk { .. } => (),
        _ => panic!("invalid response: {:?}", res),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn js_unit_test_sanity() {
    exec(
        r#"if (1 !== 1) {
            throw new Error('error, 1 !== 1');
        }"#,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn js_unit_test_encoding() {
    exec(include_str!("unit_tests/encoding.js")).await;
}
