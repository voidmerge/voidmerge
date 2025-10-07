use std::collections::HashMap;
use std::sync::Arc;

fn nonce() -> Arc<str> {
    let mut out = [0; 24];
    rand::Rng::fill(&mut rand::rng(), &mut out);
    base64::prelude::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &out,
    )
    .into()
}

pub struct Test {
    pub ctx: Arc<str>,
    #[allow(dead_code)]
    pub admin: Arc<str>,
    pub server: voidmerge::server::Server,
}

impl std::ops::Deref for Test {
    type Target = voidmerge::server::Server;

    fn deref(&self) -> &Self::Target {
        &self.server
    }
}

static BUILT: tokio::sync::OnceCell<HashMap<String, Arc<str>>> =
    tokio::sync::OnceCell::const_new();

async fn get_built(name: &str) -> Arc<str> {
    BUILT
        .get_or_init(|| async {
            tokio::process::Command::new("npm")
                .arg("ci")
                .current_dir("../../")
                .status()
                .await
                .expect("failed to run 'npm ci' command");
            tokio::process::Command::new("npm")
                .arg("run")
                .arg("build")
                .current_dir("../../")
                .status()
                .await
                .expect("failed to run 'npm ci' command");
            let mut map = HashMap::new();

            let mut dir = tokio::fs::read_dir("../../ts/test-integration/dist")
                .await
                .unwrap();
            while let Some(e) = dir.next_entry().await.unwrap() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("bundle-") && n.ends_with(".js") {
                    map.insert(
                        n.trim_start_matches("bundle-")
                            .trim_end_matches(".js")
                            .into(),
                        tokio::fs::read_to_string(e.path())
                            .await
                            .unwrap()
                            .into(),
                    );
                }
            }

            map
        })
        .await
        .get(name)
        .expect("invalid code name")
        .clone()
}

impl Test {
    pub async fn new(code_name: &str) -> Self {
        let code = get_built(code_name).await;

        let ctx = nonce();
        let admin = nonce();

        let obj = voidmerge::obj::obj_file::ObjFile::create(None)
            .await
            .unwrap();
        let js = voidmerge::js::JsExecDefault::create();
        let server = voidmerge::server::Server::new(obj, js).await.unwrap();
        server.set_sys_admin(vec![admin.clone()]).await.unwrap();
        server
            .ctx_setup_put(
                admin.clone(),
                voidmerge::server::CtxSetup {
                    ctx: ctx.clone(),
                    ctx_admin: vec![admin.clone()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        server
            .ctx_config_put(
                admin.clone(),
                voidmerge::server::CtxConfig {
                    ctx: ctx.clone(),
                    ctx_admin: vec![admin.clone()],
                    code,
                },
            )
            .await
            .unwrap();

        Self { ctx, admin, server }
    }

    pub async fn test_fn_req<R: serde::de::DeserializeOwned>(
        &self,
        body: impl serde::Serialize,
    ) -> voidmerge::Result<R> {
        let body = bytes::Bytes::copy_from_slice(
            serde_json::to_string(&body).unwrap().as_bytes(),
        );
        let res = self
            .server
            .fn_req(
                self.ctx.clone(),
                voidmerge::js::JsRequest::FnReq {
                    method: "PUT".into(),
                    path: "".into(),
                    body: Some(body),
                    headers: Default::default(),
                },
            )
            .await?;
        match res {
            voidmerge::js::JsResponse::FnResOk { status, body, .. } => {
                if status != 200.0 {
                    panic!("invalid status: {status}");
                }
                Ok(serde_json::from_slice::<R>(&body).unwrap())
            }
            oth => panic!("invalid response: {oth:?}"),
        }
    }
}
