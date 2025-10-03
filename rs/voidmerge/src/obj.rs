//! Object store type.

use crate::*;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Response type for an [Obj::list] request.
pub trait ObjListPager {
    /// Get the next group of items from the list stream.
    fn next(&mut self) -> BoxFut<'_, Result<Option<Vec<Arc<str>>>>>;
}

/// Dyn [ObjListPager] type.
pub type DynObjListPager = Box<dyn ObjListPager + 'static + Send>;

/// Low-level object store trait.
pub trait Obj: 'static + Send + Sync {
    /// Get an object by path from the store.
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<Bytes>>;

    /// List objects in the store by path prefix.
    fn list(
        &self,
        path_prefix: Arc<str>,
    ) -> BoxFut<'_, Result<DynObjListPager>>;

    /// Put an object into the store.
    fn put(&self, path: Arc<str>, obj: Bytes) -> BoxFut<'_, Result<()>>;
}

/// Dyn [Obj] type.
pub type DynObj = Arc<dyn Obj + 'static + Send + Sync>;

/// Meta-data related to an object.
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ObjMeta(pub Arc<str>);

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
        self.0.split('/').skip(1).next().unwrap_or("")
    }

    /// Get the app path associated with this meta path.
    pub fn app_path(&self) -> &str {
        self.0.split('/').skip(2).next().unwrap_or("")
    }

    /// Get the created_secs associated with this meta path.
    pub fn created_secs(&self) -> f64 {
        self.0
            .split('/')
            .skip(3)
            .next()
            .unwrap_or("")
            .parse()
            .unwrap_or(0.0)
    }

    /// Get the expires_secs associated with this meta path.
    pub fn expires_secs(&self) -> f64 {
        self.0
            .split('/')
            .skip(4)
            .next()
            .unwrap_or("")
            .parse()
            .unwrap_or(0.0)
    }
}

#[derive(Debug, Default)]
struct MemItem {
    pub meta: ObjMeta,
    pub obj: Bytes,
}

impl MemItem {
    fn parse(path: Arc<str>) -> Result<(Arc<str>, Self)> {
        let out = Self {
            meta: ObjMeta(path),
            obj: Default::default(),
        };
        let prefix = format!(
            "{}/{}/{}",
            out.meta.sys_prefix(),
            out.meta.ctx(),
            out.meta.app_path(),
        )
        .into();
        Ok((prefix, out))
    }
}

struct ObjMemInner {
    map: HashMap<Arc<str>, MemItem>,
    last_prune: std::time::Instant,
}

impl Default for ObjMemInner {
    fn default() -> Self {
        Self {
            map: Default::default(),
            last_prune: std::time::Instant::now(),
        }
    }
}

impl ObjMemInner {
    pub fn check_prune(&mut self) {
        let now = std::time::Instant::now();
        if now - self.last_prune < std::time::Duration::from_secs(5) {
            return;
        }
        self.last_prune = now;
        let now = sys_now();
        self.map.retain(|_, mem_item| {
            mem_item.meta.expires_secs() == 0.0
                || mem_item.meta.expires_secs() > now
        });
    }
}

/// An in-memory object store.
pub struct ObjMem(Mutex<ObjMemInner>);

impl ObjMem {
    /// Create a new in-memory object store.
    pub fn create() -> DynObj {
        Arc::new(Self(Default::default()))
    }
}

impl Obj for ObjMem {
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<Bytes>> {
        Box::pin(async move {
            let (prefix, _mem_item) = MemItem::parse(path.clone())?;
            let mut lock = self.0.lock().unwrap();
            lock.check_prune();
            lock.map
                .get(&prefix)
                .ok_or_else(|| {
                    Error::not_found(format!(
                        "{path} not found in object store"
                    ))
                })
                .map(|mem_item| mem_item.obj.clone())
        })
    }

    fn list(
        &self,
        path_prefix: Arc<str>,
    ) -> BoxFut<'_, Result<DynObjListPager>> {
        Box::pin(async move {
            let mut out = Vec::new();
            {
                let mut lock = self.0.lock().unwrap();
                lock.check_prune();
                for (prefix, mem_item) in lock.map.iter() {
                    if prefix.starts_with(&*path_prefix) {
                        out.push(mem_item.meta.0.clone());
                    }
                }
            }
            struct P(Option<Vec<Arc<str>>>);
            impl ObjListPager for P {
                fn next(
                    &mut self,
                ) -> BoxFut<'_, Result<Option<Vec<Arc<str>>>>> {
                    let out = Ok(self.0.take());
                    Box::pin(async move { out })
                }
            }
            let out: DynObjListPager = Box::new(P(Some(out)));
            Ok(out)
        })
    }

    fn put(&self, path: Arc<str>, obj: Bytes) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let (prefix, mut mem_item) = MemItem::parse(path)?;
            mem_item.obj = obj;
            let mut lock = self.0.lock().unwrap();
            lock.check_prune();
            let new_created_secs = mem_item.meta.created_secs();
            if let Some(prev_item) = lock.map.insert(prefix.clone(), mem_item) {
                if prev_item.meta.created_secs() >= new_created_secs {
                    // whoops, put the previous one back
                    lock.map.insert(prefix, prev_item);
                }
            }
            Ok(())
        })
    }
}

// -- pub(crate) internal types -- //

/// Pager for [ObjWrap::list].
pub struct ObjWrapListPager(DynObjListPager);

impl ObjWrapListPager {
    /// Get the next page.
    pub async fn next(&mut self) -> Result<Option<Vec<ObjMeta>>> {
        let list = match self.0.next().await? {
            None => return Ok(None),
            Some(list) => list,
        };
        Ok(Some(list.into_iter().map(ObjMeta).collect()))
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
    pub async fn get(&self, meta: ObjMeta) -> Result<Bytes> {
        self.inner.get(meta.0).await
    }

    /// List objects in the store.
    pub async fn list(&self, path_prefix: &str) -> Result<ObjWrapListPager> {
        let pager = self.inner.list(path_prefix.into()).await?;
        Ok(ObjWrapListPager(pager))
    }

    /// Put an object into the store.
    pub async fn put(&self, meta: ObjMeta, obj: Bytes) -> Result<()> {
        safe_str(&meta.app_path())
            .map_err(|err| err.with_info("invalid path"))?;
        self.inner.put(meta.0, obj).await
    }

    /// Get a single item.
    pub async fn get_single(
        &self,
        path_part: &str,
    ) -> Result<(Bytes, ObjMeta)> {
        let mut page = self.list(path_part).await?;
        while let Ok(Some(page)) = page.next().await {
            for meta in page {
                let obj = self.get(meta.clone()).await?;
                return Ok((obj, meta));
            }
        }
        Err(Error::not_found(format!("could not find {path_part}")))
    }

    /// Get the sys_setup.
    pub async fn get_sys_setup(&self) -> Result<crate::server::SysSetup> {
        use crate::server::SysSetup;

        if let Ok((sys_setup, _)) = self
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
            sys_now(),
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
        let mut page = self.inner.list(prefix).await?;
        while let Ok(Some(page)) = page.next().await {
            for path in page {
                let setup: CtxSetup =
                    self.get(ObjMeta(path)).await?.to_decode()?;
                let ctx = setup.ctx.clone();
                out.entry(ctx).or_default().0 = setup;
            }
        }

        let prefix = format!("{}/", ObjMeta::SYS_CTX_CONFIG).into();
        let mut page = self.inner.list(prefix).await?;
        while let Ok(Some(page)) = page.next().await {
            for path in page {
                let config: CtxConfig =
                    self.get(ObjMeta(path)).await?.to_decode()?;
                let ctx = config.ctx.clone();
                out.entry(ctx).or_default().1 = config;
            }
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
            sys_now(),
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
            sys_now(),
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
    async fn obj_mem() {
        let o = ObjWrap::new(ObjMem::create()).await.unwrap();

        let ctx: Arc<str> = "AAAA".into();

        o.put(
            ObjMeta::new(ObjMeta::SYS_SETUP, &ctx, "test", sys_now(), 0.0),
            Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        let mut found = Vec::new();
        let mut p = o
            .list(&format!("{}/{}/t", ObjMeta::SYS_SETUP, ctx))
            .await
            .unwrap();
        while let Ok(Some(mut v)) = p.next().await {
            found.append(&mut v);
        }
        let found = found.remove(0);

        let got = o.get(found).await.unwrap();

        assert_eq!(b"hello", got.as_ref());
    }
}
