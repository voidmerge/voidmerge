//! A server manages multiple contexts.

use crate::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn p_no(s: &Arc<str>) -> bool {
    s.is_empty()
}

fn timeout_secs() -> f64 {
    10.0
}

fn max_heap_bytes() -> usize {
    1024 * 1024 * 32
}

fn is_false(b: &bool) -> bool {
    !b
}

/// System setup information.
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SysSetup {
    /// System admin tokens.
    #[serde(rename = "x", default, skip_serializing_if = "Vec::is_empty")]
    pub sys_admin: Vec<Arc<str>>,
}

/// Context setup information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CtxSetup {
    /// The context identifier.
    #[serde(rename = "c", default, skip_serializing_if = "p_no")]
    pub ctx: Arc<str>,

    /// If this boolean is true, other properties will be ignored,
    /// and the context will be deleted.
    #[serde(rename = "d", default, skip_serializing_if = "is_false")]
    pub delete: bool,

    /// Context admin tokens.
    #[serde(rename = "x", default, skip_serializing_if = "Vec::is_empty")]
    pub ctx_admin: Vec<Arc<str>>,

    /// Timeout for function invocations.
    #[serde(rename = "t", default = "timeout_secs")]
    pub timeout_secs: f64,

    /// Max memory allowed for function invocations.
    #[serde(rename = "h", default = "max_heap_bytes")]
    pub max_heap_bytes: usize,
}

impl Default for CtxSetup {
    fn default() -> Self {
        Self {
            ctx: Default::default(),
            delete: false,
            ctx_admin: Default::default(),
            timeout_secs: timeout_secs(),
            max_heap_bytes: max_heap_bytes(),
        }
    }
}

impl CtxSetup {
    fn check(&self) -> Result<()> {
        safe_str(&self.ctx)?;
        for token in self.ctx_admin.iter() {
            safe_str(token)?;
        }
        Ok(())
    }
}

/// Context config information.
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CtxConfig {
    /// The context identifier.
    #[serde(rename = "c", default, skip_serializing_if = "p_no")]
    pub ctx: Arc<str>,

    /// Context admin tokens.
    #[serde(rename = "x", default, skip_serializing_if = "Vec::is_empty")]
    pub ctx_admin: Vec<Arc<str>>,

    /// Javascript code for the context.
    #[serde(rename = "l", default, skip_serializing_if = "p_no")]
    pub code: Arc<str>,
}

impl CtxConfig {
    fn check(&self) -> Result<()> {
        safe_str(&self.ctx)?;
        for token in self.ctx_admin.iter() {
            safe_str(token)?;
        }
        Ok(())
    }
}

/// A server manages multiple contexts.
pub struct Server {
    obj: obj::ObjWrap,
    js: js::DynJsExec,
    sys_setup: Mutex<SysSetup>,
    ctx_setup: Mutex<HashMap<Arc<str>, (CtxSetup, CtxConfig)>>,
    ctx_map: Mutex<HashMap<Arc<str>, Arc<crate::ctx::Ctx>>>,
}

impl Server {
    /// Construct a new server.
    pub async fn new(obj: obj::DynObj, js: js::DynJsExec) -> Result<Self> {
        let obj = obj::ObjWrap::new(obj).await?;

        let sys_setup = obj.get_sys_setup().await?;

        let ctx_setup = obj.list_ctx_all().await?;

        let this = Self {
            obj,
            js,
            sys_setup: Mutex::new(sys_setup),
            ctx_setup: Mutex::new(ctx_setup.clone()),
            ctx_map: Mutex::new(HashMap::new()),
        };

        for (ctx, (setup, config)) in ctx_setup {
            this.setup_context(ctx, setup, config)?;
        }

        Ok(this)
    }

    fn setup_context(
        &self,
        ctx: Arc<str>,
        setup: CtxSetup,
        config: CtxConfig,
    ) -> Result<()> {
        let obj = self.obj.clone();
        let js = self.js.clone();
        self.ctx_map.lock().unwrap().insert(
            ctx.clone(),
            Arc::new(crate::ctx::Ctx::new(ctx, setup, config, obj, js)?),
        );
        Ok(())
    }

    fn get_sys_setup(&self) -> SysSetup {
        self.sys_setup.lock().unwrap().clone()
    }

    fn get_ctx_setup(&self, ctx: &str) -> Result<(CtxSetup, CtxConfig)> {
        self.ctx_setup
            .lock()
            .unwrap()
            .get(ctx)
            .cloned()
            .ok_or_else(|| Error::not_found(format!("no context: {ctx}")))
    }

    fn check_sysadmin(&self, token: &Arc<str>) -> Result<()> {
        if !self.get_sys_setup().sys_admin.contains(token) {
            return Err(Error::unauthorized(
                "only sysadmins can perform a ctx-setup",
            ));
        }
        Ok(())
    }

    fn check_ctxadmin(
        &self,
        token: &Arc<str>,
        ctx: &Arc<str>,
    ) -> Result<(CtxSetup, CtxConfig)> {
        let (cur_setup, cur_config) = self.get_ctx_setup(ctx)?;

        if !self.get_sys_setup().sys_admin.contains(token) {
            // If we are not a sys admin, we must be a ctx admin
            if !cur_setup.ctx_admin.contains(token)
                && !cur_config.ctx_admin.contains(token)
            {
                return Err(Error::unauthorized(
                    "only sysadmins and ctxadmins can perform a ctx-config",
                ));
            }
        }

        Ok((cur_setup, cur_config))
    }

    /// Set sysadmin tokens.
    pub async fn set_sys_admin(&self, sys_admin: Vec<Arc<str>>) -> Result<()> {
        for token in sys_admin.iter() {
            safe_str(token)?;
        }
        let mut sys_setup = self.get_sys_setup();
        sys_setup.sys_admin = sys_admin;
        self.obj.set_sys_setup(sys_setup.clone()).await?;
        *self.sys_setup.lock().unwrap() = sys_setup;
        Ok(())
    }

    /// A general health check that is not context-specific.
    pub async fn health_get(&self) -> Result<()> {
        Ok(())
    }

    /// Setup a context.
    pub async fn ctx_setup_put(
        &self,
        token: Arc<str>,
        setup: CtxSetup,
    ) -> Result<()> {
        self.check_sysadmin(&token)?;

        setup.check()?;

        self.obj.set_ctx_setup(setup.clone()).await?;

        let (ctx, (ctx_setup, ctx_config)) = {
            let ctx = setup.ctx.clone();
            let mut lock = self.ctx_setup.lock().unwrap();
            let r = lock.entry(ctx.clone()).or_default();
            r.0 = setup;
            (ctx, r.clone())
        };

        self.setup_context(ctx, ctx_setup, ctx_config)?;

        Ok(())
    }

    /// Configure a context.
    pub async fn ctx_config_put(
        &self,
        token: Arc<str>,
        config: CtxConfig,
    ) -> Result<()> {
        self.check_ctxadmin(&token, &config.ctx)?;

        config.check()?;

        self.obj.set_ctx_config(config.clone()).await?;

        let (ctx, (ctx_setup, ctx_config)) = {
            let ctx = config.ctx.clone();
            let mut lock = self.ctx_setup.lock().unwrap();
            let r = lock.entry(ctx.clone()).or_default();
            r.1 = config;
            (ctx, r.clone())
        };

        self.setup_context(ctx, ctx_setup, ctx_config)?;

        Ok(())
    }

    /// List metadata from the object store.
    pub async fn obj_list_get(
        &self,
        token: Arc<str>,
        ctx: Arc<str>,
        prefix: Arc<str>,
        created_gt: f64,
        limit: u32,
    ) -> Result<Vec<crate::obj::ObjMeta>> {
        self.check_ctxadmin(&token, &ctx)?;

        let prefix =
            format!("{}/{}/{prefix}", crate::obj::ObjMeta::SYS_CTX, ctx,);

        self.obj.list(&prefix, created_gt, limit).await
    }

    /// Process a function request.
    pub async fn fn_req(
        &self,
        ctx: Arc<str>,
        req: crate::js::JsRequest,
    ) -> Result<crate::js::JsResponse> {
        let c = match self.ctx_map.lock().unwrap().get(&ctx) {
            None => {
                return Err(Error::not_found(format!(
                    "invalid context: {ctx}"
                )));
            }
            Some(c) => c.clone(),
        };
        c.fn_req(req).await
    }
}

/*
//! A server runs with a [config::Config], and manages multiple [Context]s.

use crate::*;
use context::Context;
use std::collections::HashMap;
use types::*;

/// A server runs with a [config::Config], and manages multiple [Context]s.
pub struct Server {
    client: Arc<http_client::HttpClient>,
    runtime: Arc<runtime::Runtime>,
    logic: DynModuleLogic,
    token_tracker: Arc<TokenTracker>,
    context_map: tokio::sync::Mutex<HashMap<Hash, Arc<Context>>>,
    task_list: Vec<tokio::task::AbortHandle>,
}

impl Drop for Server {
    fn drop(&mut self) {
        for task in self.task_list.drain(..) {
            task.abort();
        }
    }
}

impl Server {
    /// Construct a new [Server] with the provided [runtime::Runtime].
    pub async fn new(runtime: Arc<runtime::Runtime>) -> Result<Arc<Self>> {
        let logic = runtime
            .config()
            .logic
            .factory(runtime.config().clone())
            .await?;

        let mut task_list = Vec::new();

        let token_tracker = Arc::new(TokenTracker::default());

        for token in &runtime.config().sysadmin_tokens {
            let token = token.trim();
            if !token.is_empty() {
                token_tracker.push(Token {
                    token: token.parse::<Hash>()?,
                    valid: true,
                    expires: None,
                    is_sys_admin: true,
                    is_ctx_admin: true,
                    nonce: Default::default(),
                    access: Default::default(),
                })?;
            }
        }

        let token_tracker2 = token_tracker.clone();
        task_list.push(
            tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10))
                        .await;
                    token_tracker2.prune();
                }
            })
            .abort_handle(),
        );

        let client = Arc::new(http_client::HttpClient::new(
            Default::default(),
            runtime.sign().clone(),
        ));

        let this = Arc::new(Self {
            client,
            runtime,
            logic,
            token_tracker: token_tracker.clone(),
            context_map: Default::default(),
            task_list,
        });

        for (ctx, _meta) in this.runtime.runtime_store().context_list()? {
            let _ = this.get_or_create_context(ctx).await?;
        }

        for (token, ctx_list) in
            this.runtime.runtime_store().token_ctx_list()?
        {
            token_tracker.push(Token {
                token,
                valid: true,
                expires: None,
                is_sys_admin: false,
                is_ctx_admin: true,
                nonce: Default::default(),
                access: ctx_list.into_iter().map(|c| (c, true)).collect(),
            })?;
        }

        Ok(this)
    }

    /// Access config.
    pub fn config(&self) -> &Arc<config::Config> {
        self.runtime.config()
    }

    /// Access runtime.
    pub fn runtime(&self) -> &Arc<runtime::Runtime> {
        &self.runtime
    }

    async fn get_or_create_context(&self, ctx: Hash) -> Result<Arc<Context>> {
        use std::collections::hash_map::Entry::*;
        match self.context_map.lock().await.entry(ctx.clone()) {
            Vacant(e) => {
                let context = Context::new(
                    self.client.clone(),
                    self.runtime.clone(),
                    ctx.clone(),
                    self.logic.clone(),
                )
                .await?;
                self.runtime
                    .runtime_store()
                    .context_register(ctx, Value::map_new())
                    .await?;
                e.insert(context.clone());
                Ok(context)
            }
            Occupied(e) => Ok(e.get().clone()),
        }
    }

    async fn get_context(&self, context: &Hash) -> Result<Arc<Context>> {
        self.context_map
            .lock()
            .await
            .get(context)
            .cloned()
            .ok_or_else(|| std::io::Error::other("invalid context"))
    }

    /// Get the current public system status of a context.
    pub async fn status(&self, ctx: Hash) -> Result<Value> {
        self.get_context(&ctx).await?.status().await
    }

    /// Get an auth challenge.
    pub async fn auth_chal_req(&self) -> Result<AuthChalReq> {
        let token = Hash::nonce();
        let nonce = Hash::nonce();

        // TODO - configure?
        // only allow a short time for challenge response.
        // the expires will be upgraded if they pass the challenge.
        const CHAL_TIME: std::time::Duration =
            std::time::Duration::from_secs(20);

        self.token_tracker.push(Token {
            token: token.clone(),
            valid: false,
            expires: Some(std::time::Instant::now() + CHAL_TIME),
            is_sys_admin: false,
            is_ctx_admin: false,
            nonce: nonce.clone(),
            access: HashMap::default(),
        })?;

        Ok(AuthChalReq { token, nonce })
    }

    /// Respond to an auth challenge.
    pub async fn auth_chal_res(
        &self,
        token: Hash,
        res: AuthChalRes,
    ) -> Result<()> {
        let nonce = self.token_tracker.get_nonce(&token).ok_or_else(|| {
            std::io::ErrorKind::PermissionDenied
                .with_info("invalid token/no nonce".into())
        })?;

        if !self.runtime.sign().verify(&nonce, &res.nonce_sig) {
            return Err(std::io::ErrorKind::PermissionDenied
                .with_info("signature validation failed".into()));
        }

        // TODO - configure?
        // update our token timeout to something more reasonable.
        const TOKEN_TIME: std::time::Duration =
            std::time::Duration::from_secs(60 * 5);

        self.token_tracker
            .set_valid(&token, std::time::Instant::now() + TOKEN_TIME);

        for (ctx, _app) in res.context_access {
            // TODO - run app authentication
            self.token_tracker.grant_ctx_access(&token, ctx);
        }

        Ok(())
    }

    /// Configure a context.
    pub async fn context(
        &self,
        token: Hash,
        ctx: Hash,
        config: VmContextConfig,
    ) -> Result<()> {
        // first check permissions

        if !self.token_tracker.is_sys_admin(&token) {
            if config.delete || config.ctx_admin_tokens.is_some() {
                // must be sysadmin for these actions
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }

            if !config.force_insert.is_empty()
                && !self.token_tracker.is_ctx_admin(&token, &ctx)
            {
                // must be a ctx admin fore these actions
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
        }

        // if delete, do that first, and ignore everything else

        if config.delete {
            let _ = self.context_map.lock().await.remove(&ctx);
            return Ok(());
        }

        // make sure the context exists

        let context = self.get_or_create_context(ctx.clone()).await?;

        // set up tokens

        if let Some(token_list) = config.ctx_admin_tokens {
            for token in token_list {
                self.token_tracker.push(Token {
                    token,
                    valid: true,
                    expires: None,
                    is_sys_admin: false,
                    is_ctx_admin: true,
                    nonce: Default::default(),
                    access: maplit::hashmap! { ctx.clone() => true },
                })?;
            }
        }

        // now do the force inserts

        for bundle in config.force_insert {
            context.insert_unvalidated(bundle).await?;
        }

        // all done : )

        Ok(())
    }

    /// Validate token for a "listen" socket.
    ///
    /// This function just does the token validation, the implementor
    /// must actually set up the listening socket.
    pub async fn listen(&self, token: Hash) -> Result<()> {
        if !self.token_tracker.is_valid(&token) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }

    /// Validate tokens for a "send" request.
    ///
    /// This function just does the token validation, the implementor
    /// must actually forward the message to the listening socket.
    pub async fn send(
        &self,
        ctx: Hash,
        token: Hash,
        peer_token: Hash,
    ) -> Result<()> {
        if !self.token_tracker.has_ctx_access(&token, &ctx) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }

        if !self.token_tracker.has_ctx_access(&peer_token, &ctx) {
            return Err(std::io::ErrorKind::NotFound.into());
        }

        Ok(())
    }

    /// Put encoded+signed data to this VoidMerge server.
    pub async fn insert(
        &self,
        token: Hash,
        ctx: Hash,
        bundle: Arc<VmObjSigned>,
    ) -> Result<()> {
        if !self.token_tracker.has_ctx_access(&token, &ctx) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }

        self.get_context(&ctx).await?.insert(bundle).await?;

        Ok(())
    }

    /// Select (query) data from the server.
    pub async fn select(
        &self,
        token: Hash,
        ctx: Hash,
        select: VmSelect,
    ) -> Result<VmSelectResponse> {
        if !self.token_tracker.has_ctx_access(&token, &ctx) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }

        self.get_context(&ctx).await?.select(select).await
    }

    /// Get sysweb content.
    pub async fn get_web(
        &self,
        ctx: Hash,
        ident: Hash,
    ) -> Result<(String, Bytes)> {
        let context = self.get_context(&ctx).await?;
        let bundle = context
            .select(VmSelect {
                filter_by_types: Some(vec!["sysweb".into()]),
                filter_by_idents: Some(vec![ident]),
                return_data: Some(true),
                ..Default::default()
            })
            .await?
            .results
            .into_iter()
            .next()
            .and_then(|r| r.data)
            .ok_or_else(not_found)?;
        let app = bundle.app.as_ref().ok_or_else(not_found)?;
        let mime = match app.map_get("mime") {
            Some(Value::Str(s)) => s.to_string(),
            _ => "application/octet-stream".into(),
        };
        let data = match app.map_get("data") {
            Some(Value::Bytes(b)) => b.clone(),
            _ => return Err(not_found()),
        };
        Ok((mime, data))
    }
}

fn not_found() -> std::io::Error {
    std::io::Error::from(std::io::ErrorKind::NotFound)
}

#[derive(Debug)]
struct Token {
    /// The api token.
    pub token: Hash,

    /// If this token has been authenticated.
    pub valid: bool,

    /// When this token expires.
    pub expires: Option<std::time::Instant>,

    /// Is this a system administrator token?
    /// If so, they are allowed to create new contexts.
    pub is_sys_admin: bool,

    /// Is this a context administrator token?
    /// If so, they are allowed to force insert without validation.
    pub is_ctx_admin: bool,

    /// The nonce for a challenge-response.
    pub nonce: Hash,

    /// Access by context hash.
    pub access: HashMap<Hash, bool>,
}

#[derive(Default, Debug)]
struct TokenTracker {
    map: Mutex<HashMap<Hash, Token>>,
}

impl TokenTracker {
    pub fn push(&self, token: Token) -> Result<()> {
        use std::collections::hash_map::Entry;
        match self.map.lock().unwrap().entry(token.token.clone()) {
            Entry::Occupied(_) => {
                Err(std::io::Error::other("token already exists"))
            }
            Entry::Vacant(e) => {
                e.insert(token);
                Ok(())
            }
        }
    }

    pub fn prune(&self) {
        let now = std::time::Instant::now();
        self.map.lock().unwrap().retain(|_, t| {
            if let Some(expires) = t.expires {
                expires > now
            } else {
                true
            }
        });
    }

    pub fn get_nonce(&self, token: &Hash) -> Option<Hash> {
        self.map.lock().unwrap().get(token).map(|t| t.nonce.clone())
    }

    pub fn set_valid(&self, token: &Hash, new_expires: std::time::Instant) {
        if let Some(t) = self.map.lock().unwrap().get_mut(token) {
            t.valid = true;
            t.expires = Some(new_expires);
        }
    }

    pub fn grant_ctx_access(&self, token: &Hash, ctx: Hash) {
        if let Some(t) = self.map.lock().unwrap().get_mut(token) {
            // ignore this request if it is a ctx admin token
            // otherwise they can get access to other contexts
            if t.is_ctx_admin {
                return;
            }

            t.access.insert(ctx, true);
        }
    }

    pub fn is_sys_admin(&self, token: &Hash) -> bool {
        if let Some(t) = self.map.lock().unwrap().get(token) {
            if !t.valid {
                return false;
            }
            t.is_sys_admin
        } else {
            false
        }
    }

    pub fn is_ctx_admin(&self, token: &Hash, ctx: &Hash) -> bool {
        if let Some(t) = self.map.lock().unwrap().get(token) {
            if !t.valid {
                return false;
            }

            if t.is_sys_admin {
                return true;
            }

            if !t.is_ctx_admin {
                return false;
            }

            match t.access.get(ctx) {
                Some(r) => *r,
                None => false,
            }
        } else {
            false
        }
    }

    pub fn is_valid(&self, token: &Hash) -> bool {
        if let Some(t) = self.map.lock().unwrap().get(token) {
            t.valid
        } else {
            false
        }
    }

    pub fn has_ctx_access(&self, token: &Hash, ctx: &Hash) -> bool {
        if let Some(t) = self.map.lock().unwrap().get(token) {
            if !t.valid {
                return false;
            }

            if t.is_sys_admin {
                return true;
            }

            match t.access.get(ctx) {
                Some(r) => *r,
                None => false,
            }
        } else {
            false
        }
    }
}
*/
