use super::*;

const CTX: voidmerge::types::Hash =
    voidmerge::types::Hash::from_static(&[0, 0, 0]);

struct Test {
    task: tokio::task::JoinHandle<()>,
    #[allow(dead_code)]
    pub dir: tempfile::TempDir,
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
                cmd: Cmd::Serve(ServeArg {
                    sysadmin_tokens: vec!["bobo".into()],
                    default_context: Some(CTX.to_string()),
                    http_addr: "127.0.0.1:0".into(),
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

        Arg {
            cmd: Cmd::Context(ContextArg {
                admin: Some("bobo".into()),
                url: Some(url.clone()),
                context: CTX.to_string(),
                delete: false,
                ctx_admin_tokens: None,
                env_json_file: Some("./examples/example1-env.json".into()),
                env_append_this_pubkey: true,
                logic_utf8_single: Some("./examples/example1-logic.js".into()),
                web_root: Some("./examples".into()),
                test_server: None,
            }),
            data_dir: Some(dir.path().into()),
        }
        .exec(None)
        .await
        .unwrap();

        Ok(Self {
            task,
            dir,
            url,
            sign,
        })
    }
}

#[tokio::test]
async fn serve_and_context() -> std::io::Result<()> {
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
