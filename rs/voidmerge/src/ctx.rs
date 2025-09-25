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
    obj: crate::obj::ObjWrap,
    js: crate::js::DynJsExec,
    js_setup: crate::js::JsSetup,
}

impl Ctx {
    /// Construct a new context.
    pub fn new(
        ctx: Arc<str>,
        setup: crate::server::CtxSetup,
        config: crate::server::CtxConfig,
        obj: crate::obj::ObjWrap,
        js: crate::js::DynJsExec,
    ) -> Result<Self> {
        let js_setup = crate::js::JsSetup {
            ctx: ctx.clone(),
            timeout: std::time::Duration::from_secs_f64(setup.timeout_secs),
            heap_size: setup.max_heap_bytes,
            code: config.code.clone(),
        };
        Ok(Self {
            ctx,
            setup,
            config,
            obj,
            js,
            js_setup,
        })
    }

    /// Process a function request.
    pub async fn fn_req(
        &self,
        req: crate::js::JsRequest,
    ) -> Result<crate::js::JsResponse> {
        self.js
            .exec(self.js_setup.clone(), self.obj.clone(), req)
            .await
    }
}
