//! VoidMerge [Context]s represent spaces for sharing data
//! and connecting peers.

use crate::*;
use types::*;

/// A [Context] represents a core VoidMerge working space.
pub struct Context {
    client: Arc<http_client::HttpClient>,
    logic: DynModuleLogic,
    context: Hash,
    context_store: DynModuleContextStore,
    app_logic: Mutex<VmLogic>,
    app_env: Mutex<VmEnv>,
    sync_task: tokio::task::AbortHandle,
}

impl Drop for Context {
    fn drop(&mut self) {
        self.sync_task.abort();
    }
}

impl Context {
    /// Construct a new context instance.
    pub async fn new(
        client: Arc<http_client::HttpClient>,
        runtime: Arc<runtime::Runtime>,
        context: Hash,
        logic: DynModuleLogic,
    ) -> Result<Arc<Self>> {
        let app_logic = Mutex::new(logic.default_logic());
        let app_env = Mutex::new(Default::default());

        client.set_app_auth_data(context.clone(), Value::Unit);

        let context_store = runtime
            .config()
            .context_store
            .factory(runtime.clone(), context.clone())
            .await?;

        if let Ok(logic) = context_store
            .select(VmSelect {
                filter_by_types: Some(vec!["syslogic".into()]),
                filter_by_idents: Some(vec![Hash::from_static(b"\0\0\0")]),
                return_data: Some(true),
                ..Default::default()
            })
            .await
        {
            if let Some(l) = &logic.results.first() {
                if let Some(l) = &l.data {
                    if let Some(l) = &l.app {
                        *app_logic.lock().unwrap() = decode(&encode(&l)?)?;
                    }
                }
            }
        }

        if let Ok(env) = context_store
            .select(VmSelect {
                filter_by_types: Some(vec!["sysenv".into()]),
                filter_by_idents: Some(vec![Hash::from_static(b"\0\0\0")]),
                return_data: Some(true),
                ..Default::default()
            })
            .await
        {
            if let Some(l) = &env.results.first() {
                if let Some(l) = &l.data {
                    if let Some(l) = &l.app {
                        *app_env.lock().unwrap() = decode(&encode(l)?)?;
                    }
                }
            }
        }

        Ok(Arc::new_cyclic(|this: &std::sync::Weak<Context>| {
            let this = this.clone();
            let sync_task = tokio::task::spawn(async move {
                use rand::Rng;

                loop {
                    let pause = rand::thread_rng().gen_range(10..30);

                    tracing::debug!(pause, "sync_process: pause seconds");

                    tokio::time::sleep(std::time::Duration::from_secs(pause))
                        .await;

                    let this = match this.upgrade() {
                        None => return,
                        Some(this) => this,
                    };

                    this.sync_process().await;
                }
            })
            .abort_handle();

            Self {
                client,
                logic,
                context,
                context_store,
                app_logic,
                app_env,
                sync_task,
            }
        }))
    }

    async fn sync_process(&self) {
        let servers = self.app_env.lock().unwrap().public.servers.clone();
        tracing::debug!(?servers, "sync_process: start");

        for server in servers {
            self.sync_server(server).await;
        }
    }

    async fn sync_server(&self, server: String) {
        tracing::debug!(server, "sync_server: start");

        let remote_shorts = match self
            .client
            .select(
                &server,
                self.context.clone(),
                VmSelect {
                    return_short: Some(true),
                    ..Default::default()
                },
            )
            .await
        {
            Err(err) => {
                tracing::warn!(
                    ?err,
                    ?server,
                    "sync_server: error fetching remote shorts"
                );
                return;
            }
            Ok(shorts) => shorts
                .results
                .into_iter()
                .filter_map(|r| r.short)
                .collect::<std::collections::HashSet<_>>(),
        };

        let local_shorts = match self
            .context_store
            .select(VmSelect {
                return_short: Some(true),
                ..Default::default()
            })
            .await
        {
            Err(err) => {
                tracing::warn!(
                    ?err,
                    ?server,
                    "sync_server: error fetching local shorts"
                );
                return;
            }
            Ok(shorts) => shorts
                .results
                .into_iter()
                .filter_map(|r| r.short)
                .collect::<std::collections::HashSet<_>>(),
        };

        let diff = remote_shorts.difference(&local_shorts).collect::<Vec<_>>();

        tracing::debug!(
            remote_count = remote_shorts.len(),
            local_count = local_shorts.len(),
            diff_count = diff.len(),
            "sync_server: short diff",
        );

        for short in remote_shorts.difference(&local_shorts) {
            tracing::debug!(?short, "sync_server: fetching short");

            match self
                .client
                .select(
                    &server,
                    self.context.clone(),
                    VmSelect {
                        filter_by_shorts: Some(vec![short.clone()]),
                        return_data: Some(true),
                        ..Default::default()
                    },
                )
                .await
            {
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        ?server,
                        ?short,
                        "sync_server: error fetching remote data for short"
                    );
                    continue;
                }
                Ok(VmSelectResponse { results, .. }) => {
                    if let Some(res) = results.first() {
                        if let Some(res) = res.data.clone() {
                            tracing::debug!(
                                ?short,
                                "sync_server: inserting short"
                            );
                            if let Err(err) = self.insert(res).await {
                                tracing::warn!(
                                    ?err,
                                    ?short,
                                    "sync_server: error inserting short"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get the current public system status of this context.
    pub async fn status(&self) -> Result<Value> {
        let pub_env = self.app_env.lock().unwrap().public.clone();
        let mut out = Value::map_new();
        out.map_insert("env".into(), decode(&encode(&pub_env)?)?);
        Ok(out)
    }

    /// Insert encoded+signed data into this VoidMerge context.
    pub async fn insert(&self, dec: Arc<VmObjSigned>) -> Result<()> {
        let cur = self
            .context_store
            .select(VmSelect {
                filter_by_types: Some(vec![dec.type_.clone()]),
                filter_by_idents: Some(vec![dec.canon_ident()]),
                return_data: Some(true),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                e.with_info("checking existing type/ident entry".into())
            })?
            .results
            .into_iter()
            .next()
            .and_then(|r| r.data);

        let app_logic = self.app_logic.lock().unwrap().clone();

        // TODO - configure this
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

        let mut input = Value::map_new();
        input.map_insert("call".into(), "validate".into());
        input.map_insert("type".into(), dec.type_.clone().into());
        input.map_insert(
            "env".into(),
            decode(&encode(&*self.app_env.lock().unwrap())?)?,
        );
        input.map_insert("data".into(), decode(&encode(&dec)?)?);

        let mut result = self
            .logic
            .exec(ModuleLogicExec {
                logic: app_logic,
                system_cb: Arc::new(|m| {
                    let type_ = match m.map_get("type") {
                        Some(Value::Str(t)) => t,
                        _ => {
                            return Err(std::io::Error::other(
                                "system call did not include type property",
                            ));
                        }
                    };
                    match &**type_ {
                        "trace" => {
                            tracing::trace!(
                                system = ?m,
                                "logic system call trace",
                            );
                            Ok(Value::Unit)
                        }
                        "vmEncode" => {
                            let data =
                                m.map_get("data").unwrap_or(&Value::Unit);
                            Ok(Value::Bytes(encode(data)?))
                        }
                        "vmDecode" => match m.map_get("data") {
                            Some(Value::Bytes(b)) => decode(b),
                            _ => Err(std::io::Error::other(
                                "vmDecode expects a binary 'data' property",
                            )),
                        },
                        "utf8Encode" => match m.map_get("data") {
                            Some(Value::Str(s)) => {
                                Ok(Value::Bytes(s.as_bytes().to_vec().into()))
                            }
                            _ => Err(std::io::Error::other(
                                "utf8Encode expects a string 'data' property",
                            )),
                        },
                        "utf8Decode" => match m.map_get("data") {
                            Some(Value::Bytes(b)) => Ok(Value::Str(
                                String::from_utf8_lossy(b).into(),
                            )),
                            _ => Err(std::io::Error::other(
                                "utf8Decode expects a binary 'data' property",
                            )),
                        },
                        "randomBytes" => match m.map_get("byteLength") {
                            Some(Value::Float(f)) => {
                                use rand::Rng;
                                let len = f
                                    .floor()
                                    .clamp(0.0, 65_536.0) as usize;
                                let mut out = bytes::BytesMut::zeroed(len);
                                rand::thread_rng().fill(&mut out[..]);
                                Ok(Value::Bytes(out.freeze()))
                            }
                            _ => Err(std::io::Error::other(
                                "randomBytes expects a number 'byteLength' property",
                            )),
                        },
                        _ => Err(std::io::Error::other(format!(
                            "unhandled system call type: {type_}"
                        ))),
                    }
                }),
                input,
                timeout: Some(TIMEOUT),
            })
            .await?;

        if let Value::Str(s) = &result {
            if &**s != "unimplemented" {
                return Err(std::io::ErrorKind::InvalidData.with_info(
                    format!("invalid 'validate' return type: {result:?}"),
                ));
            }
            // treat it as valid
        } else {
            match result.map_remove("result") {
                Some(Value::Str(s)) if &*s == "valid" => (),
                _ => {
                    return Err(std::io::ErrorKind::InvalidData.with_info(
                        format!("invalid 'validate' return type: {result:?}"),
                    ));
                }
            }
        }

        // put the data in the context store
        let type_ = dec.type_.clone();
        self.context_store
            .insert(cur, dec.clone())
            .await
            .map_err(|e| {
                e.with_info(format!(
                    "inserting a {type_} type entry into the store"
                ))
            })?;

        // if the store succeeded, and it was a syslogic type,
        // actually start using the new logic
        if &*type_ == "syslogic" {
            if let Some(dec_logic) = &dec.app {
                let dec_logic = decode(&encode(dec_logic)?)?;
                *self.app_logic.lock().unwrap() = dec_logic;
            }
        } else if &*type_ == "sysenv" {
            let env: VmEnv = match &dec.app {
                None => Default::default(),
                Some(app) => decode(&encode(app)?)?,
            };
            *self.app_env.lock().unwrap() = env;
        }

        Ok(())
    }

    /// Select (query) data from the context.
    pub async fn select(&self, select: VmSelect) -> Result<VmSelectResponse> {
        self.context_store.select(select).await
    }
}
