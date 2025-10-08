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
  --code      <CODE>      : Javascript code for the context (env: VM_CODE=)
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
            args.set_default_env("code", "VM_CODE");
            args.set_default("code", "");
            Ok(Arg::CtxConfig {
                url: exp!(args, "url").into(),
                token: exp!(args, "token").into(),
                context: exp!(args, "context").into(),
                ctx_admin: args
                    .to_list_str("ctx-admin")
                    .expect("--sys-admin is required")
                    .map(|s| s.into())
                    .collect::<Vec<_>>(),
                code: exp!(args, "code").into(),
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
            help();
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
        code: Arc<str>,
    },
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
                let http_addr: std::net::SocketAddr =
                    http_addr.parse().map_err(|err| {
                        Error::other(err).with_info(
                            "failed to parse http server bind address",
                        )
                    })?;
                let (s, r) = tokio::sync::oneshot::channel();
                tokio::task::spawn(async move {
                    if let Ok(addr) = r.await {
                        println!("#vm#listening#{addr:?}#");
                    }
                });
                let server = server::Server::new(
                    obj::obj_file::ObjFile::create(store).await?,
                    js::JsExecDefault::create(),
                )
                .await?;
                server.set_sys_admin(sys_admin).await?;
                http_server::http_server(s, http_addr, server).await
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
                code,
            } => {
                let ctx_config = crate::server::CtxConfig {
                    ctx: context,
                    ctx_admin,
                    code,
                };

                let client =
                    voidmerge::http_client::HttpClient::new(Default::default());
                client.ctx_config(&url, &token, ctx_config).await
            }
        }
    }
}
