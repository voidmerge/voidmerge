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
pub trait Obj {
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

fn p_no(s: &Arc<str>) -> bool {
    s.is_empty()
}

fn ts_no(f: &f64) -> bool {
    *f != 0.0
}

/// Meta-data related to an object.
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjMeta {
    /// The path to this object.
    #[serde(rename = "p", default, skip_serializing_if = "p_no")]
    pub path: Arc<str>,

    /// Created time of this object.
    #[serde(rename = "c", default, skip_serializing_if = "ts_no")]
    pub created_secs: f64,

    /// Expires time of this object.
    #[serde(rename = "e", default, skip_serializing_if = "ts_no")]
    pub expires_secs: f64,
}

impl ObjMeta {
    /// System path: "s" for system.
    pub(crate) const SYS_PATH_SYS: &'static str = "s";

    /// System path: "c" for context.
    pub(crate) const SYS_PATH_CTX: &'static str = "c";

    /// Parse an ObjMeta from a full system path.
    pub(crate) fn with_path(
        path: &str,
    ) -> Result<(&'static str, Arc<str>, Self)> {
        use base64::prelude::*;

        let mut sys_prefix: &'static str = Default::default();
        let mut ctx = Default::default();
        let mut out = ObjMeta::default();

        let mut iter = path.split('/');

        if let Some(s) = iter.next() {
            if s == Self::SYS_PATH_SYS {
                sys_prefix = Self::SYS_PATH_SYS;
            } else if s == Self::SYS_PATH_CTX {
                sys_prefix = Self::SYS_PATH_CTX;
            } else {
                return Err(Error::other("bad object store path (sys_prefix)"));
            }
        } else {
            return Err(Error::other("bad object store path (sys_prefix)"));
        }

        if let Some(s) = iter.next() {
            ctx = s.into();
        } else {
            return Err(Error::other("bad object store path (ctx)"));
        }

        if let Some(s) = iter.next() {
            out.path = s.into();
        } else {
            return Err(Error::other("bad object store path (path)"));
        }

        if let Some(s) = iter.next() {
            if let Ok(s) = s.parse() {
                out.created_secs = s;
            } else {
                return Err(Error::other(
                    "bad object store path (created_secs)",
                ));
            }
        } else {
            return Err(Error::other("bad object store path (created_secs)"));
        }

        if let Some(s) = iter.next() {
            if let Ok(s) = s.parse() {
                out.expires_secs = s;
            } else {
                return Err(Error::other(
                    "bad object store path (expires_secs)",
                ));
            }
        } else {
            return Err(Error::other("bad object store path (expires_secs)"));
        }

        Ok((sys_prefix, ctx, out))
    }

    /// Get the full system path.
    pub(crate) fn sys_path(
        &self,
        sys_prefix: &'static str,
        ctx: Arc<str>,
    ) -> Arc<str> {
        use base64::prelude::*;
        format!(
            "{}/{}/{}/{}/{}",
            sys_prefix, ctx, self.path, self.created_secs, self.expires_secs,
        )
        .into()
    }
}

#[derive(Debug, Default)]
struct MemItem {
    pub meta: ObjMeta,
    pub obj: Bytes,
}

impl MemItem {
    fn parse(path: Arc<str>) -> Result<(Arc<str>, Self)> {
        fn it_f64<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<f64> {
            if let Some(f) = iter.next() {
                if let Ok(f) = f.parse::<f64>() {
                    return Ok(f);
                }
            }
            Err(Error::other("failed to parse f64"))
        }

        let mut out = Self::default();

        let mut iter = path.rsplitn(3, '/');
        out.meta.expires_secs = it_f64(&mut iter)?;
        out.meta.created_secs = it_f64(&mut iter)?;
        let prefix: Arc<str> = iter
            .next()
            .ok_or_else(|| Error::other("failed to parse prefix"))?
            .into();
        out.meta.path = path;
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
            mem_item.meta.expires_secs == 0.0
                || mem_item.meta.expires_secs > now
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
                        out.push(mem_item.meta.path.clone());
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
            let new_created_secs = mem_item.meta.created_secs;
            if let Some(prev_item) = lock.map.insert(prefix.clone(), mem_item) {
                if prev_item.meta.created_secs >= new_created_secs {
                    // whoops, put the previous one back
                    lock.map.insert(prefix, prev_item);
                }
            }
            Ok(())
        })
    }
}

// -- pub(crate) internal types -- //

fn u16_min() -> u16 {
    u16::MIN
}

fn u16_max() -> u16 {
    u16::MAX
}

fn timeout_s() -> f64 {
    10.0
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SysSetup {
    #[serde(rename = "x", default, skip_serializing_if = "Vec::is_empty")]
    pub sys_admin: Vec<Arc<str>>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CtxSetup {
    #[serde(rename = "x", default, skip_serializing_if = "Vec::is_empty")]
    pub ctx_admin: Vec<Arc<str>>,

    #[serde(rename = "t", default = "timeout_s")]
    pub timeout_s: f64,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CtxConfig {
    #[serde(rename = "a", default, skip_serializing_if = "HashMap::is_empty")]
    pub assets: HashMap<Arc<str>, Bytes>,
}

pub(crate) struct ObjWrapListPager(DynObjListPager);

impl ObjWrapListPager {
    pub async fn next(&mut self) -> Result<Option<Vec<ObjMeta>>> {
        let list = match self.0.next().await? {
            None => return Ok(None),
            Some(list) => list,
        };
        let mut out = Vec::with_capacity(list.len());
        for path in list {
            let (_, _, meta) = ObjMeta::with_path(&path)?;
            out.push(meta);
        }
        Ok(Some(out))
    }
}

/// Object store type.
pub(crate) struct ObjWrap {
    inner: DynObj,
    sys_setup: Mutex<SysSetup>,
}

impl ObjWrap {
    /// Constructor.
    pub async fn new(obj: DynObj) -> Result<Self> {
        let this = Self {
            inner: obj,
            sys_setup: Default::default(),
        };

        let sys_setup = if let Ok((sys_setup, _)) = this
            .get_single(ObjMeta::SYS_PATH_SYS, "s".into(), "setup")
            .await
        {
            sys_setup.to_decode()?
        } else {
            SysSetup::default()
        };

        *this.sys_setup.lock().unwrap() = sys_setup;

        Ok(this)
    }
}

impl ObjWrap {
    /// Get an object by metadata from the store.
    pub async fn get(
        &self,
        sys_prefix: &'static str,
        ctx: Arc<str>,
        meta: ObjMeta,
    ) -> Result<Bytes> {
        self.inner.get(meta.sys_path(sys_prefix, ctx)).await
    }

    /// List objects in the store.
    pub async fn list(
        &self,
        sys_prefix: &'static str,
        ctx: Arc<str>,
        path_prefix: &str,
    ) -> Result<ObjWrapListPager> {
        let prefix = format!("{}/{}/{}", sys_prefix, ctx, path_prefix).into();
        let pager = self.inner.list(prefix).await?;
        Ok(ObjWrapListPager(pager))
    }

    /// Put an object into the store.
    pub async fn put(
        &self,
        sys_prefix: &'static str,
        ctx: Arc<str>,
        meta: ObjMeta,
        obj: Bytes,
    ) -> Result<()> {
        if ctx.len() > 44 {
            return Err(Error::other("ctx too long"));
        }
        if meta.path.len() > 512 {
            return Err(Error::other("path too long"));
        }
        const OK: &str =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_~";
        for c in meta.path.chars() {
            if !OK.contains(c) {
                return Err(Error::other(format!(
                    "invalid characters in path (azAZ09-_~): {}",
                    &meta.path
                )));
            }
        }
        let path = meta.sys_path(sys_prefix, ctx).into();
        self.inner.put(path, obj).await
    }

    /// Get a single item.
    pub async fn get_single(
        &self,
        sys_prefix: &'static str,
        ctx: Arc<str>,
        path_part: &str,
    ) -> Result<(Bytes, ObjMeta)> {
        let mut page = self.list(sys_prefix, ctx.clone(), path_part).await?;
        while let Ok(Some(page)) = page.next().await {
            for meta in page {
                let obj = self.get(sys_prefix, ctx, meta.clone()).await?;
                return Ok((obj, meta));
            }
        }
        Err(Error::not_found(format!(
            "could not find {sys_prefix}/{ctx}/{path_part}"
        )))
    }

    /// Get the sys_setup.
    pub fn get_sys_setup(&self) -> SysSetup {
        self.sys_setup.lock().unwrap().clone()
    }

    /// Set the sys_setup.
    pub async fn set_sys_setup(&self, sys_setup: SysSetup) -> Result<()> {
        let meta = ObjMeta {
            path: "setup".into(),
            created_secs: sys_now(),
            ..Default::default()
        };
        self.put(
            ObjMeta::SYS_PATH_SYS,
            "s".into(),
            meta,
            Bytes::from_encode(&sys_setup)?,
        )
        .await?;
        *self.sys_setup.lock().unwrap() = sys_setup;
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
            ObjMeta::SYS_PATH_SYS,
            ctx.clone(),
            ObjMeta {
                path: "test".into(),
                created_secs: sys_now(),
                ..Default::default()
            },
            Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        let mut found = Vec::new();
        let mut p = o
            .list(ObjMeta::SYS_PATH_SYS, ctx.clone(), "t".into())
            .await
            .unwrap();
        while let Ok(Some(mut v)) = p.next().await {
            found.append(&mut v);
        }
        let found = found.remove(0);

        let got = o
            .get(ObjMeta::SYS_PATH_SYS, ctx.clone(), found)
            .await
            .unwrap();

        assert_eq!(b"hello", got.as_ref());
    }
}
