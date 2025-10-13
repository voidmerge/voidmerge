//! Context.

use crate::*;
use std::sync::Arc;

/// Context.
pub struct Ctx {
    #[allow(dead_code)]
    ctx: Arc<str>,
    #[allow(dead_code)]
    setup: crate::server::CtxSetup,
    #[allow(dead_code)]
    config: crate::server::CtxConfig,
    js_setup: crate::js::JsSetup,
}

impl Ctx {
    /// Construct a new context.
    pub fn new(
        ctx: Arc<str>,
        setup: crate::server::CtxSetup,
        config: crate::server::CtxConfig,
        runtime: Runtime,
    ) -> Result<Self> {
        let js_setup = crate::js::JsSetup {
            runtime,
            ctx: ctx.clone(),
            timeout: std::time::Duration::from_secs_f64(setup.timeout_secs),
            heap_size: setup.max_heap_bytes,
            code: config.code.clone(),
        };
        Ok(Self {
            ctx,
            setup,
            config,
            js_setup,
        })
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
