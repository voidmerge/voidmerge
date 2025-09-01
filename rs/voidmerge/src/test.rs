use crate::types::*;
use std::sync::Arc;

struct Test {
    _dir: tempfile::TempDir,
    pub token: Hash,
    pub ctx: Hash,
    pub server: Arc<crate::server::Server>,
}

impl Test {
    pub async fn new(logic: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let token: Hash = "bobo".parse().unwrap();
        let ctx = Hash::from_static(b"\0\0\0");

        let config = Arc::new(crate::config::Config {
            sysadmin_tokens: vec!["bobo".into()],
            data_dir: dir.path().into(),
            ..Default::default()
        });

        let runtime = crate::runtime::Runtime::new(config).await.unwrap();
        let server = crate::server::Server::new(runtime).await.unwrap();

        server
            .context(
                token.clone(),
                ctx.clone(),
                VmContextConfig {
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        server
            .insert(
                token.clone(),
                ctx.clone(),
                Arc::new(
                    VmObj {
                        type_: "syslogic".into(),
                        ident: Some(Hash::from_static(b"\0\0\0")),
                        deps: None,
                        ttl_s: None,
                        app: Some(
                            decode(
                                &encode(&VmLogic::Utf8Single {
                                    code: logic.into(),
                                })
                                .unwrap(),
                            )
                            .unwrap(),
                        ),
                    }
                    .sign(server.runtime().sign())
                    .unwrap(),
                ),
            )
            .await
            .unwrap();

        Self {
            _dir: dir,
            token,
            ctx,
            server,
        }
    }
}

#[tokio::test]
async fn v8_logic_invalid() {
    const LOGIC: &str = r#"
VM({
    call: 'register',
    code(i) {
        const parsed = VM({
            call: 'system',
            type: 'vmDecode',
            data: i.data.enc,
        });
        if (parsed.app === 'valid') {
            return { result: 'valid' };
        } else {
            throw new Error('invalid');
        }
    }
});
"#;

    let test = Test::new(LOGIC).await;

    test.server
        .insert(
            test.token.clone(),
            test.ctx.clone(),
            Arc::new(
                VmObj {
                    type_: "mytype".into(),
                    ident: None,
                    deps: None,
                    ttl_s: None,
                    app: Some("valid".into()),
                }
                .sign(test.server.runtime().sign())
                .unwrap(),
            ),
        )
        .await
        .unwrap();

    let err = test
        .server
        .insert(
            test.token.clone(),
            test.ctx.clone(),
            Arc::new(
                VmObj {
                    type_: "mytype".into(),
                    ident: None,
                    deps: None,
                    ttl_s: None,
                    app: Some("invalid".into()),
                }
                .sign(test.server.runtime().sign())
                .unwrap(),
            ),
        )
        .await
        .unwrap_err();
    let err = format!("{err:?}");
    assert!(err.contains("Error: invalid"));
}

#[tokio::test]
async fn v8_logic_text_encoding() {
    const LOGIC: &str = r#"
VM({
    call: 'register',
    code(_i) {
        const enc = new TextEncoder().encode("hello");
        if (!(enc instanceof Uint8Array)) {
            throw new TypeError("encode did not return Uint8Array");
        }
        const dec = new TextDecoder().decode(enc);
        if (typeof dec !== 'string') {
            throw new TypeError("decode did not return string");
        }
        if (dec !== "hello") {
            throw new Error("encode/decode round trip did not succeed");
        }
        return { result: 'valid' };
    }
});
"#;

    let test = Test::new(LOGIC).await;

    test.server
        .insert(
            test.token.clone(),
            test.ctx.clone(),
            Arc::new(
                VmObj {
                    type_: "mytype".into(),
                    ident: None,
                    deps: None,
                    ttl_s: None,
                    app: None,
                }
                .sign(test.server.runtime().sign())
                .unwrap(),
            ),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn v8_logic_vm_encoding() {
    const LOGIC: &str = r#"
VM({
    call: 'register',
    code(_i) {
        const expect = {
            null: null,
            str: "hello",
            float: 3.14,
            array: [1, 2],
            map: {a: "a", b: "b"}
        };
        const enc = VM({call:'system',type:'vmEncode',data:expect});
        if (!(enc instanceof Uint8Array)) {
            throw new TypeError("vmEncode did not return Uint8Array");
        }
        const dec = VM({call:'system',type:'vmDecode',data:enc});
        if (typeof dec !== 'object') {
            throw new TypeError("decode did not return object");
        }
        if (dec.str !== expect.str || dec.float !== expect.float || dec.array[0] !== expect.array[0] || dec.array[1] !== expect.array[1] || dec.map.a !== expect.map.a || dec.map.b !== expect.map.b) {
            throw new Error(`mismatch: expected: ${JSON.stringify(expect)}, got: ${JSON.stringify(dec)}`);
        }
        return { result: 'valid' };
    }
});
"#;

    let test = Test::new(LOGIC).await;

    test.server
        .insert(
            test.token.clone(),
            test.ctx.clone(),
            Arc::new(
                VmObj {
                    type_: "mytype".into(),
                    ident: None,
                    deps: None,
                    ttl_s: None,
                    app: None,
                }
                .sign(test.server.runtime().sign())
                .unwrap(),
            ),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn v8_logic_random() {
    const LOGIC: &str = r#"
VM({
    call: 'register',
    code(_i) {
        const rand = crypto.getRandomValues(new Uint8Array(4));
        if (!(rand instanceof Uint8Array)) {
            throw new TypeError('did not return Uint8Array');
        }
        if (rand[0] === 0 && rand[1] === 0 && rand[2] === 0 && rand[3] === 0) {
            throw new Error('unlikely!');
        }
        return { result: 'valid' };
    }
});
"#;

    let test = Test::new(LOGIC).await;

    test.server
        .insert(
            test.token.clone(),
            test.ctx.clone(),
            Arc::new(
                VmObj {
                    type_: "mytype".into(),
                    ident: None,
                    deps: None,
                    ttl_s: None,
                    app: None,
                }
                .sign(test.server.runtime().sign())
                .unwrap(),
            ),
        )
        .await
        .unwrap();
}
