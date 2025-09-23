//! Object store type.

use crate::*;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Callback for receiving metadata for object listing.
pub type ObjLowListPageCb = Arc<dyn Fn(Vec<Arc<str>>) + 'static + Send + Sync>;

/// Low-level object store trait.
pub trait ObjLow {
    /// Get an object by path from the store.
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<Bytes>>;

    /// List objects in the store by path prefix.
    fn list(
        &self,
        path_prefix: Arc<str>,
        cb: ObjLowListPageCb,
    ) -> BoxFut<'_, Result<()>>;

    /// Put an object into the store.
    fn put(&self, path: Arc<str>, obj: Bytes) -> BoxFut<'_, Result<()>>;
}

/// Dyn [ObjLow] type.
pub type DynObjLow = Arc<dyn ObjLow + 'static + Send + Sync>;

fn p_no(s: &Arc<str>) -> bool {
    s.is_empty()
}

fn ts_no(f: &f64) -> bool {
    *f != 0.0
}

/// Meta-data related to an object.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct ObjMeta {
    /// The system prefix. This doesn't get serialized into context code.
    #[serde(default, skip)]
    pub sys_prefix: &'static str,

    /// The system context. This doesn't get serialized into context code.
    #[serde(default, skip)]
    pub ctx: Bytes,

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
    pub const SYS_PATH_SYS: &'static str = "s";

    /// System path: "c" for context.
    pub const SYS_PATH_CTX: &'static str = "c";

    /// Parse an ObjMeta from a full system path.
    pub fn with_path(path: &str) -> Result<Self> {
        use base64::prelude::*;

        let mut out = ObjMeta::default();

        let mut iter = path.split('/');

        if let Some(s) = iter.next() {
            if s == Self::SYS_PATH_SYS {
                out.sys_prefix = Self::SYS_PATH_SYS;
            } else if s == Self::SYS_PATH_CTX {
                out.sys_prefix = Self::SYS_PATH_CTX;
            } else {
                return Err(Error::other("bad object store path (sys_prefix)"));
            }
        } else {
            return Err(Error::other("bad object store path (sys_prefix)"));
        }

        if let Some(s) = iter.next() {
            if let Ok(ctx) = BASE64_URL_SAFE_NO_PAD.decode(s) {
                out.ctx = Bytes::copy_from_slice(&ctx);
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

        Ok(out)
    }

    /// Get the full system path.
    pub fn sys_path(&self) -> Arc<str> {
        use base64::prelude::*;
        let ctx = BASE64_URL_SAFE_NO_PAD.encode(&self.ctx);
        format!(
            "{}/{}/{}/{}/{}/{}",
            self.sys_prefix,
            ctx,
            self.path,
            self.created_secs,
            self.expires_secs,
            self.recheck_interval_secs,
        )
        .into()
    }
}

/// Callback for receiving metadata for object listing.
pub type ObjListPageCb = Arc<dyn Fn(Vec<ObjMeta>) + 'static + Send + Sync>;

/// Object store type.
pub struct Obj(DynObjLow);

impl Obj {
    /// Get an object by metadata from the store.
    pub async fn get(&self, meta: ObjMeta) -> Result<Bytes> {
        self.0.get(meta.sys_path()).await
    }

    /// List objects in the store.
    pub async fn list(
        &self,
        sys_prefix: &'static str,
        ctx: Bytes,
        path_prefix: &str,
        cb: ObjListPageCb,
    ) -> Result<()> {
        use base64::prelude::*;
        let ctx = BASE64_URL_SAFE_NO_PAD.encode(&ctx);
        let prefix = format!("{}/{}/{}", sys_prefix, ctx, path_prefix).into();
        self.0
            .list(
                prefix,
                Arc::new(move |list| {
                    let mut out = Vec::with_capacity(list.len());
                    for path in list {
                        let meta = match ObjMeta::with_path(&path) {
                            Ok(meta) => meta,
                            Err(err) => {
                                tracing::warn!(
                                    ?err,
                                    "invalid path in obj list"
                                );
                                continue;
                            }
                        };
                        out.push(meta);
                    }
                    if !out.is_empty() {
                        cb(out);
                    }
                }),
            )
            .await
    }

    /// Put an object into the store.
    pub async fn put(&self, meta: ObjMeta, obj: Bytes) -> Result<()> {
        let path = meta.sys_path().into();
        self.0.put(path, obj).await
    }
}

/// An in-memory object store.
#[derive(Default)]
pub struct ObjMem(Mutex<HashMap<Arc<str>, Bytes>>);

impl ObjLow for ObjMem {
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<Bytes>> {
        Box::pin(async move {
            self.0.lock().unwrap().get(&path).cloned().ok_or_else(|| {
                Error::not_found(format!("{path} not found in object store"))
            })
        })
    }

    fn list(
        &self,
        path_prefix: Arc<str>,
        cb: ObjLowListPageCb,
    ) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let mut out = Vec::new();
            for p in self.0.lock().unwrap().keys() {
                if p.starts_with(&*path_prefix) {
                    out.push(p.clone());
                }
            }
            cb(out);
            Ok(())
        })
    }

    fn put(&self, path: Arc<str>, obj: Bytes) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().insert(path, obj);
            Ok(())
        })
    }
}
