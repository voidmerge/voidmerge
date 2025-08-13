use super::*;

const CTX: voidmerge::types::Hash =
    voidmerge::types::Hash::from_static(&[0, 0, 0]);

struct Test {
    _dir: tempfile::TempDir,
    task: tokio::task::JoinHandle<()>,
    pub url: String,
    pub sign: Arc<voidmerge::types::MultiSign>,
}

impl Drop for Test {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Test {
    pub async fn new() -> std::io::Result<Self> {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().into();
        let (ready_send, ready_recv) = tokio::sync::oneshot::channel();
        let task = tokio::task::spawn(async move {
            Arg {
                cmd: Cmd::ServeAndPushApp(ServeAndPushAppArg {
                    serve_arg: ServeArg {
                        sysadmin_tokens: vec!["bobo".into()],
                        default_context: Some(CTX.to_string()),
                        http_addr: "127.0.0.1:0".into(),
                    },
                    push_app_arg: PushAppArg {
                        admin: Some("bobo".into()),
                        url: "".into(),
                        context: CTX.to_string(),
                        env_json_file: Some(
                            "./examples/example1-env.json".into(),
                        ),
                        env_append_this_pubkey: true,
                        logic_utf8_single: Some(
                            "./examples/example1-logic.js".into(),
                        ),
                        web_root: Some("./examples".into()),
                    },
                }),
                data_dir: Some(data_dir),
            }
            .exec(Some(ready_send))
            .await
            .unwrap()
        });

        let url = ready_recv
            .await
            .map_err(|_| std::io::Error::other("not ready"))?;

        let runtime = voidmerge::runtime::Runtime::new(Arc::new(
            voidmerge::config::Config {
                data_dir: dir.path().into(),
                ..Default::default()
            },
        ))
        .await?;

        let sign = runtime.sign().clone();

        Ok(Self {
            _dir: dir,
            task,
            url,
            sign,
        })
    }
}

#[tokio::test]
async fn serve_and_push_app() -> std::io::Result<()> {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(
                tracing_subscriber::EnvFilter::builder()
                    .with_default_directive(
                        tracing_subscriber::filter::LevelFilter::INFO.into(),
                    )
                    .from_env_lossy(),
            )
            .compact()
            .without_time()
            .with_test_writer()
            .finish(),
    );

    let test = Test::new().await?;

    let cli = voidmerge::http_client::HttpClient::new(
        Default::default(),
        test.sign.clone(),
    );
    cli.set_api_token("bobo".parse()?);

    cli.select(
        &test.url,
        CTX.clone(),
        voidmerge::types::VmSelect {
            ..Default::default()
        },
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn ctx_admin_auth() -> std::io::Result<()> {
    let test = Test::new().await?;

    let cli = voidmerge::http_client::HttpClient::new(
        Default::default(),
        test.sign.clone(),
    );
    cli.set_app_auth_data(CTX.clone(), voidmerge::types::Value::Unit);

    cli.select(
        &test.url,
        CTX.clone(),
        voidmerge::types::VmSelect {
            ..Default::default()
        },
    )
    .await?;

    Ok(())
}
