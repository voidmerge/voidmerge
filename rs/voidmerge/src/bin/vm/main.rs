use voidmerge::*;

#[derive(Debug, clap::Parser)]
#[command(version, about)]
struct Arg {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    /// Run the VoidMerge HTTP server.
    #[cfg(feature = "http-server")]
    Serve(ServeArg),
}

#[cfg(feature = "http-server")]
#[derive(Debug, clap::Args)]
struct ServeArg {
    /// Http server address to bind.
    #[arg(long, env = "VM_HTTP_ADDR", default_value = "[::]:8080")]
    http_addr: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing::subscriber::set_global_default(
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
            .finish(),
    )
    .unwrap();

    let arg: Arg = clap::Parser::parse();

    match arg.cmd {
        #[cfg(feature = "http-server")]
        Cmd::Serve(ServeArg { http_addr }) => serve(http_addr).await,
    }
}

async fn serve(http_addr: String) -> Result<()> {
    let http_addr: std::net::SocketAddr = http_addr.parse().map_err(|err| {
        Error::other(err).with_info("failed to parse http server bind address")
    })?;
    let (s, r) = tokio::sync::oneshot::channel();
    tokio::task::spawn(async move {
        if let Ok(addr) = r.await {
            println!("#vm#listening#{addr:?}#");
        }
    });
    http_server::http_server(s, http_addr).await
}

/*
#![allow(clippy::collapsible_if)]
use std::sync::Arc;

const APP_INFO: app_dirs2::AppInfo = app_dirs2::AppInfo {
    name: "VoidMerge",
    author: "VoidMerge",
};

#[derive(Debug, clap::Parser)]
#[command(version, about)]
struct Arg {
    #[command(subcommand)]
    cmd: Cmd,

    /// Directory for storing runtime data.
    /// If not specified, a system data directory will be used.
    #[arg(long, env = "VM_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,
}

impl Arg {
    async fn exec(
        self,
        ready: Option<tokio::sync::oneshot::Sender<String>>,
    ) -> std::io::Result<()> {
        let data_dir = match self.data_dir {
            Some(data_dir) => data_dir,
            None => app_dirs2::get_app_root(
                app_dirs2::AppDataType::UserData,
                &APP_INFO,
            )
            .map_err(std::io::Error::other)?,
        };

        match self.cmd {
            Cmd::PrintPublicKeys => print_public_keys().await?,
            Cmd::Health(health_arg) => health(data_dir, health_arg).await?,
            Cmd::Serve(serve_arg) => serve(data_dir, serve_arg, ready).await,
            Cmd::Context(context_arg) => {
                context(data_dir, context_arg, ready).await?
            }
            Cmd::Backup(backup_arg) => backup(data_dir, backup_arg).await?,
            Cmd::Restore(restore_arg) => restore(data_dir, restore_arg).await?,
        }
        Ok(())
    }
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    /// Print the public keys used by this node to stderr.
    PrintPublicKeys,

    /// Execute a health check against a server.
    Health(HealthArg),

    /// Run the VoidMerge HTTP server.
    #[cfg(feature = "http-server")]
    Serve(ServeArg),

    /// Configure the specified context.
    Context(ContextArg),

    /// Backup the specified context as a canonical VoidMerge backup zipfile.
    Backup(BackupArg),

    /// Restore a VoidMerge backup zipfile into a given context..
    Restore(RestoreArg),
}

#[derive(Debug, clap::Args)]
struct HealthArg {
    /// The server url.
    #[arg(long, env = "VM_URL")]
    url: String,
}

#[derive(Debug, clap::Args)]
struct ServeArg {
    /// SysAdmin tokens to accept, these will never expire.
    /// Specify as a comma-separated list.
    #[arg(long, env = "VM_SYSADMIN_TOKENS", value_delimiter = ',')]
    sysadmin_tokens: Vec<String>,

    /// Adds a redirect at "/" to "/web/{default_context}/index.html".
    #[arg(long, env = "VM_DEFAULT_CONTEXT")]
    default_context: Option<String>,

    /// Http server address to bind.
    #[arg(long, env = "VM_HTTP_ADDR", default_value = "[::]:8080")]
    http_addr: String,
}

#[derive(Debug, clap::Args)]
struct ContextArg {
    /// The admin api token to use. If specified, client will not use
    /// challenge authentication, and instead will always pass this
    /// api token.
    #[arg(long, env = "VM_ADMIN")]
    admin: Option<String>,

    /// The context to configure.
    #[arg(long, env = "VM_CONTEXT")]
    context: String,

    /// The server url. Optional only if using --test-server.
    #[arg(long, env = "VM_URL")]
    url: Option<String>,

    /// If true, the context will be deleted. Any additionally specified
    /// context configuration will be ignored.
    #[arg(long, env = "VM_CONTEXT_DELETE")]
    delete: bool,

    /// If specified, will modify the ctx_admin tokens associated with this
    /// context.
    #[arg(long, env = "VM_CTXADMIN_TOKENS", value_delimiter = ',')]
    ctx_admin_tokens: Option<Vec<String>>,

    /// Push the given json file as a `sysenv:AAAA` entry, which will be
    /// available as the env param in logic evaluation.
    ///
    /// A string entry in the json can contain the following replacers:
    ///
    /// - `{{inc-bin <file>}}` will load the file as a binary entry.
    ///
    /// - `{{inc-str <file>}}` will load the file as a text entry.
    ///
    /// - `{{b64-bin <data>}}` will translate the inline base64url data
    ///   as a binary entry.
    ///
    /// - `{{b64-str <data>}}` will translate the inline base64url data
    ///   as a text entry.
    #[arg(long, env = "VM_ENV_JSON_FILE")]
    env_json_file: Option<std::path::PathBuf>,

    /// Artificially append this node's pubkey as a ctxadmin env item.
    #[arg(long, env = "VM_ENV_APPEND_THIS_PUBKEY")]
    env_append_this_pubkey: bool,

    /// Push the given file contents as a single utf8 syslogic item.
    #[arg(long, env = "VM_LOGIC_UTF8_SINGLE")]
    logic_utf8_single: Option<std::path::PathBuf>,

    /// Recursively upload files in this directory as sysweb items
    /// to be served at `/web/{context}/ *` paths.
    #[arg(long, env = "VM_WEB_ROOT")]
    web_root: Option<std::path::PathBuf>,

    /// Run a new test server at the configured socket address.
    /// (E.g. `--test-server 127.0.0.1:0`)
    #[arg(long, env = "VM_TEST_SERVER")]
    test_server: Option<String>,
}

#[derive(Debug, clap::Args)]
struct BackupArg {
    /// The admin api token to use. If specified, client will not use
    /// challenge authentication, and instead will always pass this
    /// api token.
    #[arg(long, env = "VM_ADMIN")]
    admin: Option<String>,

    /// The server url.
    #[arg(long, env = "VM_URL")]
    url: String,

    /// The context to back up.
    #[arg(long, env = "VM_CONTEXT")]
    context: String,

    /// The filename to write. Defaults to `vm-backup-(ctx)-(time).zip`.
    #[arg(long, env = "VM_OUTPUT")]
    output: Option<std::path::PathBuf>,
}

#[derive(Debug, clap::Args)]
struct RestoreArg {
    /// The admin api token to use. If specified, client will not use
    /// challenge authentication, and instead will always pass this
    /// api token.
    #[arg(long, env = "VM_ADMIN")]
    admin: Option<String>,

    /// The server url.
    #[arg(long, env = "VM_URL")]
    url: String,

    /// The context to back up.
    #[arg(long, env = "VM_CONTEXT")]
    context: String,

    /// The filename to read.
    #[arg(long, env = "VM_INPUT")]
    input: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing::subscriber::set_global_default(
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
            .finish(),
    )
    .unwrap();

    let arg: Arg = clap::Parser::parse();

    arg.exec(None).await
}

async fn print_public_keys() -> std::io::Result<()> {
    let config = voidmerge::config::Config {
        ..Default::default()
    };
    let runtime = voidmerge::runtime::Runtime::new(Arc::new(config)).await?;
    for pk in runtime.sign().public_keys() {
        eprintln!("{pk}");
    }
    Ok(())
}

async fn health(
    data_dir: std::path::PathBuf,
    health_arg: HealthArg,
) -> std::io::Result<()> {
    // don't actually need runtime/sign for this call,
    // but it's how the client is currently set up.
    let config = voidmerge::config::Config {
        data_dir,
        ..Default::default()
    };
    let runtime = voidmerge::runtime::Runtime::new(Arc::new(config)).await?;
    let client = voidmerge::http_client::HttpClient::new(
        Default::default(),
        runtime.sign().clone(),
    );
    client.health(&health_arg.url).await
}

async fn serve(
    data_dir: std::path::PathBuf,
    serve_arg: ServeArg,
    ready: Option<tokio::sync::oneshot::Sender<String>>,
) {
    serve_err(data_dir, serve_arg, ready)
        .await
        .expect("error running server");
}

async fn serve_err(
    data_dir: std::path::PathBuf,
    serve_arg: ServeArg,
    ready: Option<tokio::sync::oneshot::Sender<String>>,
) -> std::io::Result<()> {
    let default_context = match serve_arg.default_context {
        Some(c) => Some(c.parse()?),
        None => None,
    };

    let config = voidmerge::config::Config {
        sysadmin_tokens: serve_arg.sysadmin_tokens,
        default_context,
        http_addr: serve_arg.http_addr,
        data_dir,
        ..Default::default()
    };

    let runtime = voidmerge::runtime::Runtime::new(Arc::new(config)).await?;

    tracing::debug!(?runtime);

    let server = voidmerge::server::Server::new(runtime).await?;
    let server = voidmerge::http_server::HttpServer::new(server).await?;
    let addr = *server.bound_addr();

    tracing::info!(?addr, "listening");
    eprintln!("#voidmerged#listening:{:?}#", addr);

    if let Some(ready) = ready {
        let _ = ready.send(format!("http://{addr:?}"));
    }

    server.wait().await;
    Ok(())
}

async fn context(
    data_dir: std::path::PathBuf,
    context_arg: ContextArg,
    ready: Option<tokio::sync::oneshot::Sender<String>>,
) -> std::io::Result<()> {
    let config = voidmerge::config::Config {
        data_dir: data_dir.clone(),
        ..Default::default()
    };
    let runtime = voidmerge::runtime::Runtime::new(Arc::new(config)).await?;
    tracing::debug!(?runtime);

    let ContextArg {
        admin,
        mut url,
        context,
        delete,
        ctx_admin_tokens,
        env_json_file,
        env_append_this_pubkey,
        logic_utf8_single,
        web_root,
        test_server,
    } = context_arg;

    let context: voidmerge::types::Hash = context.parse()?;

    let client = voidmerge::http_client::HttpClient::new(
        Default::default(),
        runtime.sign().clone(),
    );
    if let Some(admin) = &admin {
        let admin: voidmerge::types::Hash = admin.parse()?;
        client.set_api_token(admin);
    }

    if delete {
        tracing::info!("deleting context..");

        client
            .context(
                &url.expect("must pass in url if using delete"),
                context.clone(),
                voidmerge::types::VmContextConfig {
                    delete: true,
                    ..Default::default()
                },
            )
            .await?;

        // If deleting, that's all we do
        return Ok(());
    }

    let mut test_server_task = None;

    if let Some(test_server) = test_server {
        let (s, r) = tokio::sync::oneshot::channel();
        let sysadmin_tokens = if let Some(admin) = &admin {
            vec![admin.clone()]
        } else {
            vec![]
        };
        let default_context = Some(context.to_string());

        test_server_task = Some(tokio::task::spawn(async move {
            serve(
                data_dir,
                ServeArg {
                    sysadmin_tokens,
                    default_context,
                    http_addr: test_server,
                },
                Some(s),
            )
            .await
        }));

        url = Some(r.await.map_err(|_| {
            std::io::Error::from(std::io::ErrorKind::BrokenPipe)
        })?);

        // make sure we can actually connect
        let mut is_healthy = false;
        for _ in 0..40 {
            if client.health(url.as_ref().unwrap()).await.is_ok() {
                is_healthy = true;
                break;
            }
        }
        if !is_healthy {
            return Err(std::io::Error::other("failed to bind test-server"));
        }
    }

    let url = url.expect("either specify a --url or --test-server");

    if let Some(ctx_admin_tokens) = ctx_admin_tokens {
        let mut tokens = Vec::with_capacity(ctx_admin_tokens.len());
        for t in ctx_admin_tokens {
            tokens.push(t.parse()?);
        }

        tracing::info!("configuring ctx_admin tokens..");

        // configure the ctxadmin tokens
        client
            .context(
                &url,
                context.clone(),
                voidmerge::types::VmContextConfig {
                    ctx_admin_tokens: Some(tokens),
                    ..Default::default()
                },
            )
            .await?;
    }

    let ts = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap()
        .as_secs_f64();

    if let Some(env_json_file) = env_json_file {
        use voidmerge::types::*;

        tracing::info!("pushing sysenv from {env_json_file:?}..");
        let dir = env_json_file.parent().ok_or_else(|| {
            std::io::Error::other(
                "could not get env_json_file containing directory",
            )
        })?;
        let env = tokio::fs::read_to_string(&env_json_file).await?;
        let env: Value =
            serde_json::from_str(&env).map_err(std::io::Error::other)?;
        let env = env.transform(&mut ValueTxFromHuman::new(dir)).await?;
        let mut env: VmEnv = decode(&encode(&env)?)?;
        if env_append_this_pubkey {
            env.private
                .ctxadmin_pubkeys
                .push(runtime.sign().public_keys());
        }
        let env: Value = decode(&encode(&env)?)?;
        let env = VmObj {
            type_: "sysenv".into(),
            ident: Some((&b"\0\0\0"[..]).into()),
            deps: None,
            ttl_s: None,
            app: Some(env),
        };

        tracing::info!(?env);

        let bundle = env.sign(runtime.sign())?;

        // inject the env without validation
        client
            .context(
                &url,
                context.clone(),
                voidmerge::types::VmContextConfig {
                    force_insert: vec![bundle.into()],
                    ..Default::default()
                },
            )
            .await?;
    }

    if let Some(logic_utf8_single) = logic_utf8_single {
        tracing::info!("pushing syslogic from {logic_utf8_single:?}..");

        let code = tokio::fs::read_to_string(logic_utf8_single).await?;

        // TODO - add ts to enc logic
        let app = voidmerge::types::decode(&voidmerge::types::encode(
            &voidmerge::types::VmLogic::Utf8Single { code: code.into() },
        )?)?;

        let enc = voidmerge::types::VmObj {
            type_: "syslogic".into(),
            ident: Some((&b"\0\0\0"[..]).into()),
            deps: None,
            ttl_s: None,
            app: Some(app),
        };

        let bundle = enc.sign(runtime.sign())?;

        // inject the logic without validation
        client
            .context(
                &url,
                context.clone(),
                voidmerge::types::VmContextConfig {
                    force_insert: vec![bundle.into()],
                    ..Default::default()
                },
            )
            .await?;
    }

    if let Some(web_root) = web_root {
        let mut files = Vec::new();
        rec_file(web_root, "/".into(), &mut files).await?;

        for (path, data) in files {
            let mime = match mime_guess::from_path(&path).first() {
                Some(mime) => mime.to_string(),
                None => "application/octet-stream".into(),
            };
            let path = path
                .to_str()
                .ok_or_else(|| std::io::Error::other("invalid utf8 path"))?;
            let ident = path.as_bytes().into();

            tracing::info!("pushing sysweb to {path:?} ({ident}, {mime})..");

            let mut app = voidmerge::types::Value::map_new();
            app.map_insert("ts".into(), ts.into());
            app.map_insert("data".into(), data.into());
            app.map_insert("mime".into(), mime.into());

            let enc = voidmerge::types::VmObj {
                type_: "sysweb".into(),
                ident: Some(ident),
                deps: None,
                ttl_s: None,
                app: Some(app),
            };

            let bundle = enc.sign(runtime.sign())?;

            // inject the web file without validation
            client
                .context(
                    &url,
                    context.clone(),
                    voidmerge::types::VmContextConfig {
                        force_insert: vec![bundle.into()],
                        ..Default::default()
                    },
                )
                .await?;
        }
    }

    eprintln!("#voidmerged#context_config_complete#");
    if let Some(ready) = ready {
        let _ = ready.send(url);
    }

    if let Some(test_server_task) = test_server_task {
        test_server_task.await?;
    }

    Ok(())
}

fn rec_file(
    p: std::path::PathBuf,
    d: std::path::PathBuf,
    o: &mut Vec<(std::path::PathBuf, bytes::Bytes)>,
) -> voidmerge::types::BoxFut<'_, std::io::Result<()>> {
    Box::pin(async move {
        let mut read = tokio::fs::read_dir(&p).await?;
        while let Some(e) = read.next_entry().await? {
            let file_path = d.join(e.file_name());
            let t = e.file_type().await?;
            if t.is_dir() {
                rec_file(p.join(e.file_name()), file_path, o).await?;
            } else {
                let data = tokio::fs::read(e.path()).await?.into();
                o.push((file_path, data));
            }
        }
        Ok(())
    })
}

async fn backup(
    data_dir: std::path::PathBuf,
    backup_arg: BackupArg,
) -> std::io::Result<()> {
    let config = voidmerge::config::Config {
        data_dir,
        ..Default::default()
    };
    let runtime = voidmerge::runtime::Runtime::new(Arc::new(config)).await?;
    tracing::debug!(?runtime);

    let BackupArg {
        admin,
        url,
        context,
        output,
    } = backup_arg;

    let context: voidmerge::types::Hash = context.parse()?;

    let client = voidmerge::http_client::HttpClient::new(
        Default::default(),
        runtime.sign().clone(),
    );
    if let Some(admin) = &admin {
        let admin: voidmerge::types::Hash = admin.parse()?;
        client.set_api_token(admin);
    }

    tracing::info!("Selecting all server shorts...");

    let all = client
        .select(
            &url,
            context.clone(),
            voidmerge::types::VmSelect {
                return_short: Some(true),
                ..Default::default()
            },
        )
        .await?;

    tracing::info!("Found {} shorts on server.", all.count);

    let output = output.unwrap_or_else(|| {
        format!(
            "vm-backup-{context}-{}.zip",
            std::time::SystemTime::UNIX_EPOCH
                .elapsed()
                .unwrap()
                .as_millis()
        )
        .into()
    });

    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut file = zip::ZipWriter::new(file);

    for (i, short) in
        all.results.into_iter().filter_map(|r| r.short).enumerate()
    {
        tracing::info!(
            "Downloading {}/{} content for {short}...",
            i + 1,
            all.count
        );

        let content = client
            .select(
                &url,
                context.clone(),
                voidmerge::types::VmSelect {
                    filter_by_shorts: Some(vec![short.clone()]),
                    return_data: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        if let Some(content) = content.results.first() {
            if let Some(content) = &content.data {
                let type_ = content.type_.clone();
                let ident = content.canon_ident();
                let enc = voidmerge::types::encode(&content)?;
                let len = enc.len();

                file = tokio::task::spawn_blocking(move || {
                    use std::io::Write;
                    file.start_file(
                        format!("{}-{}.vm", type_, ident),
                        zip::write::SimpleFileOptions::default(),
                    )?;
                    file.write_all(&enc)?;
                    std::io::Result::Ok(file)
                })
                .await??;

                tracing::info!(
                    "Stored {} bytes for {}:{}",
                    len,
                    content.type_,
                    content.canon_ident(),
                );
            }
        }
    }

    Ok(())
}

async fn restore(
    data_dir: std::path::PathBuf,
    restore_arg: RestoreArg,
) -> std::io::Result<()> {
    use std::io::Read;

    let config = voidmerge::config::Config {
        data_dir,
        ..Default::default()
    };
    let runtime = voidmerge::runtime::Runtime::new(Arc::new(config)).await?;
    tracing::debug!(?runtime);

    let RestoreArg {
        admin,
        url,
        context,
        input,
    } = restore_arg;

    let context: voidmerge::types::Hash = context.parse()?;

    let client = voidmerge::http_client::HttpClient::new(
        Default::default(),
        runtime.sign().clone(),
    );
    if let Some(admin) = &admin {
        let admin: voidmerge::types::Hash = admin.parse()?;
        client.set_api_token(admin);
    }

    let file = std::fs::OpenOptions::new().read(true).open(input)?;
    let file = zip::ZipArchive::new(file)?;

    async fn read_by_index(
        mut f: zip::ZipArchive<std::fs::File>,
        idx: usize,
    ) -> std::io::Result<(zip::ZipArchive<std::fs::File>, bytes::Bytes)> {
        tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            {
                let mut read = f.by_index(idx)?;
                tracing::info!(name = ?read.name(), "inserting...");
                read.read_to_end(&mut out)?;
            }
            Ok((f, out.into()))
        })
        .await?
    }

    async fn read_by_name(
        f: zip::ZipArchive<std::fs::File>,
        name: &str,
    ) -> std::io::Result<(zip::ZipArchive<std::fs::File>, Option<bytes::Bytes>)>
    {
        if let Some(idx) = f.index_for_name(name) {
            read_by_index(f, idx).await.map(|(f, b)| (f, Some(b)))
        } else {
            Ok((f, None))
        }
    }

    let (file, sysenv) = read_by_name(file, "sysenv-AAAA.vm").await?;

    if let Some(sysenv) = sysenv {
        client.insert(&url, context.clone(), sysenv).await?;
    }

    let (mut file, syslogic) = read_by_name(file, "syslogic-AAAA.vm").await?;

    if let Some(syslogic) = syslogic {
        client.insert(&url, context.clone(), syslogic).await?;
    }

    for i in 0..file.len() {
        let (tmp, data) = read_by_index(file, i).await?;
        file = tmp;
        client.insert(&url, context.clone(), data).await?;
    }

    Ok(())
}

#[cfg(test)]
mod test;
*/
