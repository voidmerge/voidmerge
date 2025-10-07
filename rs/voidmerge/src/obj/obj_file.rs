//! File-backed object store.

use crate::obj::*;
use std::sync::Mutex;

/// File-backed object store.
pub struct ObjFile {
    root: std::path::PathBuf,
    inner: Mutex<Inner>,
    task: tokio::task::AbortHandle,
    tempdir: Option<tempfile::TempDir>,
}

impl Drop for ObjFile {
    fn drop(&mut self) {
        self.task.abort();
        if let Some(tempdir) = self.tempdir.take()
            && let Err(err) = tempdir.close()
        {
            tracing::error!(?err, "error cleaning tempdir on ObjFile drop");
        }
    }
}

impl ObjFile {
    /// Construct a new file-backed object store.
    ///
    /// If root is `None`, a tempdir will be used.
    pub async fn create(root: Option<std::path::PathBuf>) -> Result<DynObj> {
        let mut tempdir = None;

        let root = if let Some(root) = root {
            root
        } else {
            let td = tempfile::tempdir()?;
            let root = td.path().into();
            tempdir = Some(td);
            root
        };

        let out: DynObj = Arc::new_cyclic(|this: &std::sync::Weak<ObjFile>| {
            let this = this.clone();
            let task = tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10))
                        .await;
                    if let Some(this) = this.upgrade() {
                        this.inner.lock().unwrap().prune();
                    } else {
                        return;
                    }
                }
            })
            .abort_handle();
            Self {
                root,
                inner: Mutex::new(Inner::new()),
                task,
                tempdir,
            }
        });
        Ok(out)
    }
}

impl Obj for ObjFile {
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<Bytes>> {
        Box::pin(async move {
            let data_path = self.inner.lock().unwrap().get(ObjMeta(path))?;
            Ok(tokio::fs::read(data_path).await?.into())
        })
    }

    fn list(
        &self,
        path_prefix: Arc<str>,
    ) -> BoxFut<'_, Result<DynObjListPager>> {
        Box::pin(async move {
            let list = self.inner.lock().unwrap().list(path_prefix);
            struct P(std::collections::VecDeque<Arc<str>>);
            impl ObjListPager for P {
                fn next(
                    &mut self,
                ) -> BoxFut<'_, Result<Option<Vec<Arc<str>>>>> {
                    Box::pin(async move {
                        if self.0.is_empty() {
                            return Ok(None);
                        }
                        Ok(Some(
                            self.0
                                .drain(..std::cmp::min(50, self.0.len()))
                                .collect(),
                        ))
                    })
                }
            }
            let p: DynObjListPager = Box::new(P(list));
            Ok(p)
        })
    }

    fn put(&self, meta: Arc<str>, data: Bytes) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            use base64::prelude::*;
            use sha2::{Digest, Sha256};

            let meta = ObjMeta(meta);

            let sys_prefix = meta.sys_prefix();
            safe_str(sys_prefix)?;
            let ctx = meta.ctx();
            safe_str(ctx)?;
            let app_path = meta.app_path();
            safe_str(app_path)?;

            let prefix: Arc<str> =
                format!("{sys_prefix}/{ctx}/{app_path}").into();

            let mut hasher = Sha256::new();
            hasher.update(meta.as_bytes());
            hasher.update(&data);
            let hash = BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());

            let mut iter = hash.chars();
            let h1 = format!("a{}a", iter.next().unwrap());
            let h2 = format!("a{}a", iter.next().unwrap());

            let dir = std::path::PathBuf::from(&self.root)
                .join(sys_prefix)
                .join(ctx)
                .join(h1)
                .join(h2);

            tokio::fs::create_dir_all(&dir).await?;

            let meta_path = dir.join(format!("meta-{hash}"));
            tokio::fs::write(&meta_path, meta.as_bytes()).await?;

            let data_path = dir.join(format!("data-{hash}"));
            tokio::fs::write(&data_path, data).await?;

            // finally if all the writes succeeded, update our map
            self.inner.lock().unwrap().put(prefix, meta, data_path);

            Ok(())
        })
    }
}

struct Item {
    pub meta: ObjMeta,
    pub data_path: std::path::PathBuf,
}

struct Inner(HashMap<Arc<str>, Item>);

impl Inner {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn prune(&mut self) {
        let now = sys_now();
        self.0.retain(|_, v| {
            let x = v.meta.expires_secs();
            x == 0.0 || x > now
        });
    }

    pub fn get(&self, meta: ObjMeta) -> Result<std::path::PathBuf> {
        let sys_prefix = meta.sys_prefix();
        let ctx = meta.ctx();
        let app_path = meta.app_path();

        let prefix = format!("{sys_prefix}/{ctx}/{app_path}");

        if let Some(item) = self.0.get(prefix.as_str())
            && item.meta == meta
        {
            return Ok(item.data_path.clone());
        }

        Err(Error::not_found(format!("Could not locate meta: {meta}")))
    }

    pub fn list(
        &self,
        prefix: Arc<str>,
    ) -> std::collections::VecDeque<Arc<str>> {
        let mut out = std::collections::VecDeque::new();
        for (k, v) in self.0.iter() {
            if k.starts_with(&*prefix) {
                out.push_back(v.meta.0.clone());
            }
        }
        out
    }

    pub fn put(
        &mut self,
        prefix: Arc<str>,
        meta: ObjMeta,
        data_path: std::path::PathBuf,
    ) {
        let now = sys_now();
        let mx = meta.expires_secs();
        if mx > 0.0 && mx < now {
            return;
        }
        let created = meta.created_secs();
        if let Some(orig) =
            self.0.insert(prefix.clone(), Item { meta, data_path })
        {
            let ox = orig.meta.expires_secs();
            if ox > 0.0 && ox < now {
                return;
            }
            if orig.meta.created_secs() >= created {
                // whoops, put the original back
                self.0.insert(prefix, orig);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn obj_file_simple() {
        let of = ObjFile::create(None).await.unwrap();

        of.put(
            "c/AAAA/bob/1.0/0.0".into(),
            bytes::Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        let mut iter = of.list("c/AAAA/b".into()).await.unwrap();
        let mut list = iter.next().await.unwrap().unwrap();
        assert_eq!(1, list.len());

        let item = list.remove(0);
        assert_eq!("c/AAAA/bob/1.0/0.0", &*item);

        let got = of.get("c/AAAA/bob/1.0/0.0".into()).await.unwrap();
        assert_eq!(&b"hello"[..], &got[..]);
    }
}
