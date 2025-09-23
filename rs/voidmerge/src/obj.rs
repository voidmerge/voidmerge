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
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
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

    /// recheck interval of this object.
    #[serde(rename = "r", default, skip_serializing_if = "ts_no")]
    pub recheck_interval_secs: f64,
}

impl ObjMeta {
    /// System path: "s" for system.
    pub(crate) const SYS_PATH_SYS: &'static str = "s";

    /// System path: "c" for context.
    pub(crate) const SYS_PATH_CTX: &'static str = "c";

    /// Parse an ObjMeta from a full system path.
    pub(crate) fn with_path(path: &str) -> Result<(&'static str, Bytes, Self)> {
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
            if let Ok(x) = BASE64_URL_SAFE_NO_PAD.decode(s) {
                ctx = Bytes::copy_from_slice(&x);
            } else {
                return Err(Error::other("bad object store path (ctx)"));
            }
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

        if let Some(s) = iter.next() {
            if let Ok(s) = s.parse() {
                out.recheck_interval_secs = s;
            } else {
                return Err(Error::other(
                    "bad object store path (recheck_interval_secs)",
                ));
            }
        } else {
            return Err(Error::other(
                "bad object store path (recheck_interval_secs)",
            ));
        }

        Ok((sys_prefix, ctx, out))
    }

    /// Get the full system path.
    pub(crate) fn sys_path(
        &self,
        sys_prefix: &'static str,
        ctx: Bytes,
    ) -> Arc<str> {
        use base64::prelude::*;
        let ctx = BASE64_URL_SAFE_NO_PAD.encode(&ctx);
        format!(
            "{}/{}/{}/{}/{}/{}",
            sys_prefix,
            ctx,
            self.path,
            self.created_secs,
            self.expires_secs,
            self.recheck_interval_secs,
        )
        .into()
    }
}

#[derive(Debug, Default)]
struct MemItem {
    pub path: Arc<str>,
    pub created_secs: f64,
    pub expires_secs: f64,
    pub recheck_interval_secs: f64,
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

        let mut iter = path.rsplitn(4, '/');
        out.recheck_interval_secs = it_f64(&mut iter)?;
        out.expires_secs = it_f64(&mut iter)?;
        out.created_secs = it_f64(&mut iter)?;
        let prefix: Arc<str> = iter
            .next()
            .ok_or_else(|| Error::other("failed to parse prefix"))?
            .into();
        out.path = path;
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
            mem_item.expires_secs == 0.0 || mem_item.expires_secs > now
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
                        out.push(mem_item.path.clone());
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
            let new_created_secs = mem_item.created_secs;
            if let Some(prev_item) = lock.map.insert(prefix.clone(), mem_item) {
                if prev_item.created_secs >= new_created_secs {
                    // whoops, put the previous one back
                    lock.map.insert(prefix, prev_item);
                }
            }
            Ok(())
        })
    }
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

/// Callback for receiving metadata for object listing.
pub(crate) type ObjWrapListPageCb =
    Arc<dyn Fn(Vec<(&'static str, Bytes, ObjMeta)>) + 'static + Send + Sync>;

/// Object store type.
pub(crate) struct ObjWrap(DynObj);

impl ObjWrap {
    /// Constructor.
    pub fn new(obj: DynObj) -> Self {
        Self(obj)
    }
}

impl ObjWrap {
    /// Get an object by metadata from the store.
    pub async fn get(
        &self,
        sys_prefix: &'static str,
        ctx: Bytes,
        meta: ObjMeta,
    ) -> Result<Bytes> {
        self.0.get(meta.sys_path(sys_prefix, ctx)).await
    }

    /// List objects in the store.
    pub async fn list(
        &self,
        sys_prefix: &'static str,
        ctx: Bytes,
        path_prefix: &str,
    ) -> Result<ObjWrapListPager> {
        use base64::prelude::*;
        let ctx = BASE64_URL_SAFE_NO_PAD.encode(&ctx);
        let prefix = format!("{}/{}/{}", sys_prefix, ctx, path_prefix).into();
        let pager = self.0.list(prefix).await?;
        Ok(ObjWrapListPager(pager))
    }

    /// Put an object into the store.
    pub async fn put(
        &self,
        sys_prefix: &'static str,
        ctx: Bytes,
        meta: ObjMeta,
        obj: Bytes,
    ) -> Result<()> {
        let path = meta.sys_path(sys_prefix, ctx).into();
        self.0.put(path, obj).await
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn obj_mem() {
        let o = ObjWrap::new(ObjMem::create());

        const CTX: Bytes = Bytes::from_static(b"\0\0\0");

        o.put(
            ObjMeta::SYS_PATH_SYS,
            CTX.clone(),
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
            .list(ObjMeta::SYS_PATH_SYS, CTX.clone(), "t".into())
            .await
            .unwrap();
        while let Ok(Some(mut v)) = p.next().await {
            found.append(&mut v);
        }
        let found = found.remove(0);

        let got = o
            .get(ObjMeta::SYS_PATH_SYS, CTX.clone(), found)
            .await
            .unwrap();

        assert_eq!(b"hello", got.as_ref());
    }
}
