use std::sync::Arc;
use voidmerge::*;

fn help() {
    println!(
        r#"
VoidMerge P2p in Easy Mode

Usage: vm <COMMAND> <OPTIONS>

help -h --help            : Print this help

serve                     : Run the VoidMerge HTTP server
  --sys-admin <SYS_ADMIN> : SysAdmin tokens to set during startup
                            (env: VM_SYS_ADMIN_TOKENS=, comma delimited)
  --http-addr <HTTP_ADDR> : Http server address to bind (env: VM_HTTP_ADDR=)
                            (def: '[::]:8080')
  --store <PATH>          : Path location for object store file persistance.
                            (env: VM_STORE=) (def: use a temp dir)

test                      : Run a test server (sysadmin: 'test', ctx: 'test')
  --http-addr <HTTP_ADDR> : Http server address to bind (env: VM_HTTP_ADDR=)
                            (def: '[::]:8080')
  --code-file <PATH>      : Javascript code for the context (env: VM_CODE=)

health                    : Execute a health check against a server
  --url       <URL>       : The server url (env: VM_URL=)

ctx-setup                 : Setup a context (sysadmin)
  --url       <URL>       : The server url (env: VM_URL=)
  --token     <TOKEN>     : The sysadmin api token to use (env: VM_TOKEN=)
  --context   <CONTEXT>   : The context to configure (env: VM_CTX=)
  --delete                : If this flag is set, delete the context
                            Other properties will be ignored (env: VM_DELETE=)
  --ctx-admin <TOKENS>    : CtxAdmin tokens to setup in the context
                            (env: VM_CTX_ADMIN_TOKENS=, comma delimited)
  --timeout-secs          : Timeout for functions (env: VM_TIMEOUT_SECS=)
                            (def: '10.0')
  --max-heap-bytes        : Max memory for functions (env: VM_MAX_HEAP_BYTES)
                            (def: '33554432')

ctx-config                : Configure a context (ctxadmin)
  --url       <URL>       : The server url (env: VM_URL=)
  --token     <TOKEN>     : The ctxadmin api token to use (env: VM_TOKEN=)
  --context   <CONTEXT>   : The context to configure (env: VM_CTX=)
  --ctx-admin <TOKENS>    : CtxAdmin tokens to setup in the context
                            (env: VM_CTX_ADMIN_TOKENS=, comma delimited)
  --code-file <PATH>      : Javascript code for the context (env: VM_CODE=)

obj-list                  : List objects in a context store (ctxadmin)
  --url       <URL>       : The server url (env: VM_URL=)
  --token     <TOKEN>     : The ctxadmin api token to use (env: VM_TOKEN=)
  --context   <CONTEXT>   : The context to configure (env: VM_CTX=)
  --prefix    <PREFIX>    : The appPathPrefix to filter by (env: VM_PREFIX=)
  --created-gt <NUMBER>   : Filter by items with created_secs larger than the
                            supplied number. (env: VM_CREATED_GT=) (def: 0.0)
  --limit     <NUMBER>    : Limit response to provided number. (env: VM_LIMIT=)
                            (def: list all items in the store)

obj-get                   : Get an object from a context store (ctxadmin)
                            Will print the meta path to stderr
                            Will print the data content to stdout
  --url       <URL>       : The server url (env: VM_URL=)
  --token     <TOKEN>     : The ctxadmin api token to use (env: VM_TOKEN=)
  --context   <CONTEXT>   : The context to configure (env: VM_CTX=)
  --app-path  <APP_PATH>  : The appPath to fetch (env: VM_APP_PATH=)

obj-put                   : Put an object into the context store (ctxadmin)
                            Reads data from stdin
  --url       <URL>       : The server url (env: VM_URL=)
  --token     <TOKEN>     : The ctxadmin api token to use (env: VM_TOKEN=)
  --context   <CONTEXT>   : The context to configure (env: VM_CTX=)
  --app-path  <APP_PATH>  : The appPath to store (env: VM_APP_PATH=)
  --create    <TIMESTAMP> : The createdSecs to store (env: VM_CREATE=)
  --expire    <TIMESTAMP> : The expiresSecs to store (env: VM_EXPIRE=)

"#
    );
}

fn def_split_env(
    args: &mut minimist::Minimist,
    key: &str,
    env: impl Into<std::ffi::OsString>,
) {
    if let Some(val) = std::env::var_os(env.into()) {
        let r = args.entry(key.into()).or_default();
        for val in val.to_string_lossy().split(',') {
            r.push(val.into());
        }
    }
}

fn arg_parse() -> Result<Arg> {
    let mut args = minimist::Minimist::parse(std::env::args_os().skip(1));

    let mut cmd = args
        .to_one_str(minimist::Minimist::POS)
        .unwrap_or_else(|| "help".into());

    if args.as_flag("h") || args.as_flag("help") {
        cmd = "help".into();
    }

    macro_rules! exp {
        ($a:ident, $t:literal) => {
            $a.to_one_str($t).ok_or_else(|| {
                Error::invalid(concat!(
                    "Argument Error: --",
                    $t,
                    " is required"
                ))
            })?
        };
    }

    macro_rules! exp_path {
        ($a:ident, $t:literal) => {
            $a.as_one_path($t).ok_or_else(|| {
                Error::invalid(concat!(
                    "Argument Error: --",
                    $t,
                    " is required"
                ))
            })?
        };
    }

    match cmd.as_ref() {
        "help" => Ok(Arg::Help),
        "serve" => {
            def_split_env(&mut args, "sys-admin", "VM_SYS_ADMIN_TOKENS");
            args.entry("sys-admin".into()).or_default();
            args.set_default_env("http-addr", "VM_HTTP_ADDR");
            args.set_default("http-addr", "[::]:8080");
            args.set_default_env("store", "VM_STORE");
            Ok(Arg::Serve {
                sys_admin: args
                    .to_list_str("sys-admin")
                    .expect("--sys-admin is required")
                    .map(|s| s.into())
                    .collect::<Vec<_>>(),
                http_addr: exp!(args, "http-addr").into(),
                store: args.as_one_path("store").map(|p| p.to_owned()),
            })
        }
        "test" => {
            args.set_default_env("http-addr", "VM_HTTP_ADDR");
            args.set_default("http-addr", "[::]:8080");
            args.set_default_env("code-file", "VM_CODE");
            Ok(Arg::Test {
                http_addr: exp!(args, "http-addr").into(),
                code_file: exp_path!(args, "code-file").into(),
            })
        }
        "health" => {
            args.set_default_env("url", "VM_URL");
            Ok(Arg::Health {
                url: exp!(args, "url").into(),
            })
        }
        "ctx-setup" => {
            args.set_default_env("url", "VM_URL");
            args.set_default_env("token", "VM_TOKEN");
            args.set_default_env("context", "VM_CTX");
            args.set_default_env("delete", "VM_DELETE");
            def_split_env(&mut args, "ctx-admin", "VM_CTX_ADMIN_TOKENS");
            args.entry("ctx-admin".into()).or_default();
            args.set_default_env("timeout-secs", "VM_TIMEOUT_SECS");
            args.set_default("timeout-secs", "10.0");
            args.set_default_env("max-heap-bytes", "VM_MAX_HEAP_BYTES");
            args.set_default("max-heap-bytes", "33554432");
            Ok(Arg::CtxSetup {
                url: exp!(args, "url").into(),
                token: exp!(args, "token").into(),
                context: exp!(args, "context").into(),
                delete: args.as_flag("delete"),
                ctx_admin: args
                    .to_list_str("ctx-admin")
                    .expect("--sys-admin is required")
                    .map(|s| s.into())
                    .collect::<Vec<_>>(),
                timeout_secs: exp!(args, "timeout-secs")
                    .parse()
                    .map_err(Error::other)?,
                max_heap_bytes: exp!(args, "max-heap-bytes")
                    .parse()
                    .map_err(Error::other)?,
            })
        }
        "ctx-config" => {
            args.set_default_env("url", "VM_URL");
            args.set_default_env("token", "VM_TOKEN");
            args.set_default_env("context", "VM_CTX");
            def_split_env(&mut args, "ctx-admin", "VM_CTX_ADMIN_TOKENS");
            args.entry("ctx-admin".into()).or_default();
            args.set_default_env("code-file", "VM_CODE");
            Ok(Arg::CtxConfig {
                url: exp!(args, "url").into(),
                token: exp!(args, "token").into(),
                context: exp!(args, "context").into(),
                ctx_admin: args
                    .to_list_str("ctx-admin")
                    .expect("--sys-admin is required")
                    .map(|s| s.into())
                    .collect::<Vec<_>>(),
                code_file: exp_path!(args, "code-file").into(),
            })
        }
        "obj-list" => {
            args.set_default_env("url", "VM_URL");
            args.set_default_env("token", "VM_TOKEN");
            args.set_default_env("context", "VM_CTX");
            args.set_default_env("prefix", "VM_PREFIX");
            args.set_default("prefix", "");
            args.set_default_env("created-gt", "VM_CREATED_GT");
            args.set_default("created-gt", "0.0");
            args.set_default_env("limit", "VM_LIMIT");
            args.set_default("limit", "4294967295");
            Ok(Arg::ObjList {
                url: exp!(args, "url").into(),
                token: exp!(args, "token").into(),
                context: exp!(args, "context").into(),
                prefix: exp!(args, "prefix").into(),
                created_gt: exp!(args, "created-gt")
                    .parse()
                    .map_err(Error::other)?,
                limit: exp!(args, "limit").parse().map_err(Error::other)?,
            })
        }
        "obj-get" => {
            args.set_default_env("url", "VM_URL");
            args.set_default_env("token", "VM_TOKEN");
            args.set_default_env("context", "VM_CTX");
            args.set_default_env("app-path", "VM_APP_PATH");
            args.set_default("app-path", "");
            Ok(Arg::ObjGet {
                url: exp!(args, "url").into(),
                token: exp!(args, "token").into(),
                context: exp!(args, "context").into(),
                app_path: exp!(args, "app-path").into(),
            })
        }
        "obj-put" => {
            args.set_default_env("url", "VM_URL");
            args.set_default_env("token", "VM_TOKEN");
            args.set_default_env("context", "VM_CTX");
            args.set_default_env("app-path", "VM_APP_PATH");
            args.set_default("app-path", "");
            args.set_default_env("create", "VM_CREATE");
            args.set_default("create", safe_now().to_string());
            args.set_default_env("expire", "VM_EXPIRE");
            args.set_default("expire", "0.0");
            Ok(Arg::ObjPut {
                url: exp!(args, "url").into(),
                token: exp!(args, "token").into(),
                context: exp!(args, "context").into(),
                app_path: exp!(args, "app-path").into(),
                create: exp!(args, "create").parse().map_err(Error::other)?,
                expire: exp!(args, "expire").parse().map_err(Error::other)?,
            })
        }
        unk => Err(Error::other(format!("unrecognised command: {unk}"))),
    }
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

    let arg = match arg_parse() {
        Ok(arg) => arg,
        Err(err) => {
            eprintln!("\n-----\n{err}\n-----");
            eprintln!("\n`vm --help` for additional info");
            std::process::exit(1);
        }
    };

    arg.exec().await
}

enum Arg {
    Help,
    Serve {
        sys_admin: Vec<Arc<str>>,
        http_addr: String,
        store: Option<std::path::PathBuf>,
    },
    Test {
        http_addr: String,
        code_file: std::path::PathBuf,
    },
    Health {
        url: String,
    },
    CtxSetup {
        url: String,
        token: Arc<str>,
        context: Arc<str>,
        delete: bool,
        ctx_admin: Vec<Arc<str>>,
        timeout_secs: f64,
        max_heap_bytes: usize,
    },
    CtxConfig {
        url: String,
        token: Arc<str>,
        context: Arc<str>,
        ctx_admin: Vec<Arc<str>>,
        code_file: std::path::PathBuf,
    },
    ObjList {
        url: String,
        token: Arc<str>,
        context: Arc<str>,
        prefix: Arc<str>,
        created_gt: f64,
        limit: u32,
    },
    ObjGet {
        url: String,
        token: Arc<str>,
        context: Arc<str>,
        app_path: Arc<str>,
    },
    ObjPut {
        url: String,
        token: Arc<str>,
        context: Arc<str>,
        app_path: Arc<str>,
        create: f64,
        expire: f64,
    },
}

async fn serve(
    s: tokio::sync::oneshot::Sender<std::net::SocketAddr>,
    sys_admin: Vec<Arc<str>>,
    http_addr: String,
    store: Option<std::path::PathBuf>,
) -> Result<()> {
    let http_addr: std::net::SocketAddr = http_addr.parse().map_err(|err| {
        Error::other(err).with_info("failed to parse http server bind address")
    })?;
    let server = server::Server::new(
        obj::obj_file::ObjFile::create(store).await?,
        js::JsExecDefault::create(),
    )
    .await?;
    server.set_sys_admin(sys_admin).await?;
    http_server::http_server(s, http_addr, server).await
}

impl Arg {
    async fn exec(self) -> Result<()> {
        match self {
            Self::Help => {
                help();
                Ok(())
            }
            Self::Serve {
                sys_admin,
                http_addr,
                store,
            } => {
                let (s, r) = tokio::sync::oneshot::channel();
                tokio::task::spawn(async move {
                    if let Ok(addr) = r.await {
                        eprintln!("#vm#listening#{addr:?}#");
                    }
                });
                serve(s, sys_admin, http_addr, store).await
            }
            Self::Test {
                http_addr,
                code_file,
            } => {
                let code: Arc<str> =
                    tokio::fs::read_to_string(code_file).await?.into();

                let (s, r) = tokio::sync::oneshot::channel();
                tokio::task::spawn(async move {
                    // await server start
                    let addr = match r.await {
                        Ok(addr) => addr,
                        Err(err) => {
                            panic!("failed to start test server: {err:?}")
                        }
                    };

                    let url = format!("http://{addr:?}");

                    // check health
                    let client = voidmerge::http_client::HttpClient::new(
                        Default::default(),
                    );
                    let mut is_healthy = false;
                    for _ in 0..10 {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            100,
                        ))
                        .await;
                        if client.health(&url).await.is_ok() {
                            is_healthy = true;
                            break;
                        }
                    }
                    if !is_healthy {
                        panic!(
                            "failed to get healthy response from test server"
                        );
                    }

                    // setup context
                    if let Err(err) = client
                        .ctx_setup(
                            &url,
                            "test",
                            crate::server::CtxSetup {
                                ctx: "test".into(),
                                delete: false,
                                ctx_admin: vec!["test".into()],
                                timeout_secs: 10.0,
                                max_heap_bytes: 33554432,
                            },
                        )
                        .await
                    {
                        panic!("failed to setup test server context: {err:?}");
                    }

                    // configure context
                    if let Err(err) = client
                        .ctx_config(
                            &url,
                            "test",
                            crate::server::CtxConfig {
                                ctx: "test".into(),
                                ctx_admin: vec!["test".into()],
                                code,
                            },
                        )
                        .await
                    {
                        panic!("failed to setup test server context: {err:?}");
                    }

                    // okay, we're running!
                    eprintln!("#vm#listening#{addr:?}#");
                });
                serve(s, vec!["test".into()], http_addr, None).await
            }
            Self::Health { url } => {
                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                client.health(&url).await
            }
            Self::CtxSetup {
                url,
                token,
                context,
                delete,
                ctx_admin,
                timeout_secs,
                max_heap_bytes,
            } => {
                let ctx_setup = crate::server::CtxSetup {
                    ctx: context,
                    delete,
                    ctx_admin,
                    timeout_secs,
                    max_heap_bytes,
                };

                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                client.ctx_setup(&url, &token, ctx_setup).await
            }
            Self::CtxConfig {
                url,
                token,
                context,
                ctx_admin,
                code_file,
            } => {
                let code = tokio::fs::read_to_string(code_file).await?.into();

                let ctx_config = crate::server::CtxConfig {
                    ctx: context,
                    ctx_admin,
                    code,
                };

                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                client.ctx_config(&url, &token, ctx_config).await
            }
            Self::ObjList {
                url,
                token,
                context,
                prefix,
                mut created_gt,
                mut limit,
            } => {
                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                let mut count = 0;
                while limit > 1000 {
                    let next_count = std::cmp::min(1000, limit);
                    limit -= next_count;
                    let res = client
                        .obj_list(
                            &url, &token, &context, &prefix, created_gt,
                            next_count,
                        )
                        .await?;
                    if res.is_empty() {
                        break;
                    }
                    for r in res {
                        let created_secs = r.created_secs();
                        if created_secs > created_gt {
                            created_gt = created_secs;
                        }
                        count += 1;
                        println!("{r}");
                    }
                }
                eprintln!("#vm#list-count#{count}#");
                Ok(())
            }
            Self::ObjGet {
                url,
                token,
                context,
                app_path,
            } => {
                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                let (meta, data) =
                    client.obj_get(&url, &token, &context, &app_path).await?;
                eprintln!("#vm#meta#{meta}#");
                use tokio::io::AsyncWriteExt;
                tokio::io::stdout().write_all(&data).await?;
                Ok(())
            }
            Self::ObjPut {
                url,
                token,
                context,
                app_path,
                create,
                expire,
            } => {
                use tokio::io::AsyncReadExt;
                let mut data = Vec::new();
                tokio::io::stdin().read_to_end(&mut data).await?;
                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                let meta = client
                    .obj_put(
                        &url,
                        &token,
                        &context,
                        &app_path,
                        create,
                        expire,
                        data.into(),
                    )
                    .await?;
                eprintln!("#vm#meta#{meta}#");
                Ok(())
            }
        }
    }
}
