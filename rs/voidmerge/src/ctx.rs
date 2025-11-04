//! Context.

use crate::*;
use std::sync::Arc;

/// Context.
pub struct Ctx {
    this: Weak<Self>,
    #[allow(dead_code)]
    ctx: Arc<str>,
    #[allow(dead_code)]
    setup: crate::server::CtxSetup,
    #[allow(dead_code)]
    config: crate::server::CtxConfig,
    js_setup: crate::js::JsSetup,
    cron_interval_secs: Option<f64>,
    task: tokio::task::AbortHandle,
}

impl Drop for Ctx {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Ctx {
    /// Construct a new context.
    pub async fn new(
        ctx: Arc<str>,
        setup: crate::server::CtxSetup,
        config: crate::server::CtxConfig,
        runtime: Runtime,
    ) -> Result<Arc<Self>> {
        let js_setup = crate::js::JsSetup {
            runtime,
            ctx: ctx.clone(),
            timeout: std::time::Duration::from_secs_f64(setup.timeout_secs),
            heap_size: setup.max_heap_bytes,
            code: config.code.clone(),
            env: config.code_env.clone(),
        };
        let mut this = Self {
            this: Weak::new(),
            ctx,
            setup,
            config,
            js_setup,
            cron_interval_secs: None,
            task: tokio::task::spawn(async move {}).abort_handle(),
        };
        this.code_config().await?;
        let this = Arc::new_cyclic(move |weak_this| {
            let weak_this = weak_this.clone();
            this.this = weak_this.clone();
            if let Some(int) = this.cron_interval_secs {
                this.task = tokio::task::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs_f64(
                            int,
                        ))
                        .await;
                        if let Some(this) = weak_this.upgrade() {
                            let _ = this.cron_req().await;
                        } else {
                            break;
                        }
                    }
                })
                .abort_handle();
            }
            this
        });
        Ok(this)
    }

    async fn code_config(&mut self) -> Result<()> {
        if let Ok(crate::js::JsResponse::CodeConfigResOk {
            cron_interval_secs,
        }) = self
            .js_setup
            .runtime
            .js()?
            .exec(self.js_setup.clone(), crate::js::JsRequest::CodeConfigReq)
            .await
        {
            self.cron_interval_secs = cron_interval_secs;
        }
        Ok(())
    }

    async fn cron_req(&self) -> Result<()> {
        self.js_setup
            .runtime
            .js()?
            .exec(self.js_setup.clone(), crate::js::JsRequest::CronReq)
            .await?;
        Ok(())
    }

    /// Process an ObjCheck request.
    pub async fn obj_check_req(
        &self,
        meta: crate::obj::ObjMeta,
        data: bytes::Bytes,
    ) -> Result<()> {
        let res = self
            .js_setup
            .runtime
            .js()?
            .exec(
                self.js_setup.clone(),
                crate::js::JsRequest::ObjCheckReq { data, meta },
            )
            .await?;
        match res {
            crate::js::JsResponse::ObjCheckResOk => Ok(()),
            _ => Err(Error::other("invalid ObjCheck response")),
        }
    }

    /// Process a function request.
    pub async fn fn_req(
        &self,
        req: crate::js::JsRequest,
    ) -> Result<crate::js::JsResponse> {
        self.js_setup
            .runtime
            .js()?
            .exec(self.js_setup.clone(), req)
            .await
    }
}
