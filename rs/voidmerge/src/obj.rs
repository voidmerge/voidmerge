//! Object store type.

use crate::*;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;

pub mod obj_file;

/// Low-level object store trait.
pub trait Obj: 'static + Send + Sync {
    /// Get an object by path from the store.
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<(Arc<str>, Bytes)>>;

    /// List objects in the store by path prefix.
    fn list(
        &self,
        path_prefix: Arc<str>,
        created_gt: f64,
        limit: u32,
    ) -> BoxFut<'_, Result<Vec<Arc<str>>>>;

    /// Put an object into the store.
    fn put(&self, path: Arc<str>, obj: Bytes) -> BoxFut<'_, Result<()>>;
}

/// Dyn [Obj] type.
pub type DynObj = Arc<dyn Obj + 'static + Send + Sync>;

/// Meta-data related to an object.
#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Hash,
)]
#[serde(transparent)]
pub struct ObjMeta(pub Arc<str>);

impl std::ops::Deref for ObjMeta {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for ObjMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<R: AsRef<str>> From<R> for ObjMeta {
    fn from(r: R) -> Self {
        Self(r.as_ref().into())
    }
}

impl ObjMeta {
    /// System path: "s" for system setup.
    pub(crate) const SYS_SETUP: &'static str = "s";

    /// System path: "x" for context setup.
    pub(crate) const SYS_CTX_SETUP: &'static str = "x";

    /// System path: "d" for context config.
    pub(crate) const SYS_CTX_CONFIG: &'static str = "d";

    /// System path: "c" for context.
    pub(crate) const SYS_CTX: &'static str = "c";

    /// Create a new meta path from components.
    pub(crate) fn new(
        sys_prefix: &'static str,
        ctx: &str,
        app_path: &str,
        created_secs: f64,
        expires_secs: f64,
    ) -> Self {
        Self(
            format!(
                "{}/{}/{}/{}/{}",
                sys_prefix, ctx, app_path, created_secs, expires_secs,
            )
            .into(),
        )
    }

    /// Create a new context meta path from components.
    pub fn new_context(
        ctx: &str,
        app_path: &str,
        created_secs: f64,
        expires_secs: f64,
    ) -> Self {
        Self::new(Self::SYS_CTX, ctx, app_path, created_secs, expires_secs)
    }

    /// Get the sys_prefix associated with this meta path.
    pub(crate) fn sys_prefix(&self) -> &'static str {
        match self.0.split('/').next() {
            Some(Self::SYS_SETUP) => Self::SYS_SETUP,
            Some(Self::SYS_CTX_SETUP) => Self::SYS_CTX_SETUP,
            Some(Self::SYS_CTX_CONFIG) => Self::SYS_CTX_CONFIG,
            _ => Self::SYS_CTX,
        }
    }

    /// Get the ctx associated with this meta path.
    pub fn ctx(&self) -> &str {
        self.0.split('/').nth(1).unwrap_or("")
    }

    /// Get the app path associated with this meta path.
    pub fn app_path(&self) -> &str {
        self.0.split('/').nth(2).unwrap_or("")
    }

    /// Get the created_secs associated with this meta path.
    pub fn created_secs(&self) -> f64 {
        self.0
            .split('/')
            .nth(3)
            .unwrap_or("")
            .parse()
            .unwrap_or(0.0)
    }

    /// Get the expires_secs associated with this meta path.
    pub fn expires_secs(&self) -> f64 {
        self.0
            .split('/')
            .nth(4)
            .unwrap_or("")
            .parse()
            .unwrap_or(0.0)
    }
}

/// Object store type.
#[derive(Clone)]
pub struct ObjWrap {
    inner: DynObj,
}

impl ObjWrap {
    /// Constructor.
    pub async fn new(obj: DynObj) -> Result<Self> {
        let this = Self { inner: obj };

        Ok(this)
    }
}

impl ObjWrap {
    /// Get an object by metadata from the store.
    pub async fn get(&self, meta: ObjMeta) -> Result<(ObjMeta, Bytes)> {
        self.inner
            .get(meta.0)
            .await
            .map(|(meta, data)| (ObjMeta(meta), data))
    }

    /// List objects in the store.
    pub async fn list(
        &self,
        path_prefix: &str,
        created_gt: f64,
        limit: u32,
    ) -> Result<Vec<ObjMeta>> {
        Ok(self
            .inner
            .list(path_prefix.into(), created_gt, limit)
            .await?
            .into_iter()
            .map(ObjMeta)
            .collect())
    }

    /// Put an object into the store.
    pub async fn put(&self, meta: ObjMeta, obj: Bytes) -> Result<()> {
        safe_str(meta.app_path())
            .map_err(|err| err.with_info("invalid path"))?;
        self.inner.put(meta.0, obj).await
    }

    /// Get a single item.
    pub async fn get_single(
        &self,
        path_part: &str,
    ) -> Result<(ObjMeta, Bytes)> {
        let mut res = self.list(path_part, 0.0, 1).await?;
        if !res.is_empty() {
            return self.get(res.remove(0)).await;
        }
        Err(Error::not_found(format!("could not find {path_part}")))
    }

    /// Get the sys_setup.
    pub async fn get_sys_setup(&self) -> Result<crate::server::SysSetup> {
        use crate::server::SysSetup;

        if let Ok((_, sys_setup)) = self
            .get_single(&format!(
                "{}/{}/setup",
                ObjMeta::SYS_SETUP,
                ObjMeta::SYS_SETUP
            ))
            .await
        {
            sys_setup.to_decode()
        } else {
            Ok(SysSetup::default())
        }
    }

    /// Set the sys_setup.
    pub async fn set_sys_setup(
        &self,
        sys_setup: crate::server::SysSetup,
    ) -> Result<()> {
        let meta = ObjMeta::new(
            ObjMeta::SYS_SETUP,
            ObjMeta::SYS_SETUP,
            "setup",
            safe_now(),
            0.0,
        );
        self.put(meta, Bytes::from_encode(&sys_setup)?).await?;
        Ok(())
    }

    /// List all configured ctx setups and configs.
    pub async fn list_ctx_all(
        &self,
    ) -> Result<
        HashMap<Arc<str>, (crate::server::CtxSetup, crate::server::CtxConfig)>,
    > {
        use crate::server::{CtxConfig, CtxSetup};

        let mut out: HashMap<Arc<str>, (CtxSetup, CtxConfig)> = HashMap::new();

        let prefix = format!("{}/", ObjMeta::SYS_CTX_SETUP).into();
        let page = self.inner.list(prefix, 0.0, u32::MAX).await?;
        for path in page {
            let setup: CtxSetup =
                self.get(ObjMeta(path)).await?.1.to_decode()?;
            let ctx = setup.ctx.clone();
            out.entry(ctx).or_default().0 = setup;
        }

        let prefix = format!("{}/", ObjMeta::SYS_CTX_CONFIG).into();
        let page = self.inner.list(prefix, 0.0, u32::MAX).await?;
        for path in page {
            let config: CtxConfig =
                self.get(ObjMeta(path)).await?.1.to_decode()?;
            let ctx = config.ctx.clone();
            out.entry(ctx).or_default().1 = config;
        }

        Ok(out)
    }

    /// Set a ctx_setup.
    pub async fn set_ctx_setup(
        &self,
        ctx_setup: crate::server::CtxSetup,
    ) -> Result<()> {
        let meta = ObjMeta::new(
            ObjMeta::SYS_CTX_SETUP,
            &ctx_setup.ctx,
            "setup",
            safe_now(),
            0.0,
        );
        self.put(meta, Bytes::from_encode(&ctx_setup)?).await?;
        Ok(())
    }

    /// Set a ctx_config.
    pub async fn set_ctx_config(
        &self,
        ctx_config: crate::server::CtxConfig,
    ) -> Result<()> {
        let meta = ObjMeta::new(
            ObjMeta::SYS_CTX_CONFIG,
            &ctx_config.ctx,
            "config",
            safe_now(),
            0.0,
        );
        self.put(meta, Bytes::from_encode(&ctx_config)?).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn obj_wrap() {
        let o = ObjWrap::new(obj_file::ObjFile::create(None).await.unwrap())
            .await
            .unwrap();

        let ctx: Arc<str> = "AAAA".into();

        o.put(
            ObjMeta::new(ObjMeta::SYS_SETUP, &ctx, "test", safe_now(), 0.0),
            Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        let mut found = o
            .list(&format!("{}/{}/t", ObjMeta::SYS_SETUP, ctx), 0.0, 1)
            .await
            .unwrap();
        let found = found.remove(0);

        let got = o.get(found).await.unwrap().1;

        assert_eq!(b"hello", got.as_ref());
    }
}
