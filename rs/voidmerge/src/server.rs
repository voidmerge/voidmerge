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
            format!("{}/{}/{prefix}", crate::obj::ObjMeta::SYS_CTX, ctx);

        self.obj.list(&prefix, created_gt, limit).await
    }

    /// Get an item from the object store.
    pub async fn obj_get_get(
        &self,
        token: Arc<str>,
        ctx: Arc<str>,
        app_path: String,
    ) -> Result<(crate::obj::ObjMeta, bytes::Bytes)> {
        self.check_ctxadmin(&token, &ctx)?;

        let meta =
            crate::obj::ObjMeta::new_context(&ctx, &app_path, 0.0, 0.0, 0.0);

        self.obj.get(meta).await
    }

    /// Put an item into the object store.
    pub async fn obj_put_put(
        &self,
        token: Arc<str>,
        ctx: Arc<str>,
        app_path: String,
        created_secs: f64,
        expires_secs: f64,
        data: bytes::Bytes,
    ) -> Result<crate::obj::ObjMeta> {
        self.check_ctxadmin(&token, &ctx)?;

        let meta = crate::obj::ObjMeta::new_context(
            &ctx,
            &app_path,
            created_secs,
            expires_secs,
            data.len() as f64,
        );

        // TODO ObjCheck

        self.obj.put(meta.clone(), data).await?;
        Ok(meta)
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
