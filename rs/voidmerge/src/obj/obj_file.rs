//! File-backed object store.

use crate::memindex::*;
use crate::obj::*;
use std::sync::Mutex;

#[derive(Clone)]
struct Info {
    pub meta_path: std::path::PathBuf,
    pub data_path: std::path::PathBuf,
}

/// File-backed object store.
pub struct ObjFile {
    root: std::path::PathBuf,
    index: Mutex<MemIndex<Info>>,
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
    pub async fn create(root: Option<std::path::PathBuf>) -> Result<ObjWrap> {
        let mut tempdir = None;

        let root = if let Some(root) = root {
            root
        } else {
            let td = tempfile::tempdir()?;
            let root = td.path().into();
            tempdir = Some(td);
            root
        };

        let out = Arc::new_cyclic(|this: &std::sync::Weak<ObjFile>| {
            let this = this.clone();
            let task = tokio::task::spawn(async move {
                let mut last_meter = std::time::Instant::now();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10))
                        .await;
                    if let Some(this) = this.upgrade() {
                        let path_list = {
                            let mut lock = this.index.lock().unwrap();
                            lock.prune();
                            lock.get_delete()
                        };
                        destroy(path_list).await;

                        let now = std::time::Instant::now();
                        let diff_sec = (now - last_meter).as_secs_f64();
                        if diff_sec > 60.0 {
                            last_meter = now;
                            let diff_min = diff_sec / 60.0;
                            let map = this.index.lock().unwrap().meter();
                            for (ctx, storage) in map {
                                crate::meter::meter_obj_store_byte_min(
                                    &ctx,
                                    (storage as f64 * diff_min) as u128,
                                );
                            }
                        }
                    } else {
                        return;
                    }
                }
            })
            .abort_handle();
            Self {
                root,
                index: Mutex::new(MemIndex::default()),
                task,
                tempdir,
            }
        });

        out.load().await?;

        let out: DynObj = out;

        let out = ObjWrap::new(out);

        Ok(out)
    }

    async fn load(&self) -> Result<()> {
        let mut dir = tokio::fs::read_dir(&self.root).await?;
        while let Some(e) = dir.next_entry().await? {
            if e.file_type().await?.is_dir() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.len() == 1 {
                    self.load_sys_prefix(e.path(), name.into()).await?;
                }
            }
        }

        Ok(())
    }

    async fn load_sys_prefix(
        &self,
        path: std::path::PathBuf,
        sys_prefix: Arc<str>,
    ) -> Result<()> {
        let mut dir = tokio::fs::read_dir(&path).await?;
        while let Some(e) = dir.next_entry().await? {
            if e.file_type().await?.is_dir() {
                let name = e.file_name().to_string_lossy().to_string();
                self.load_ctx(e.path(), sys_prefix.clone(), name.into())
                    .await?;
            }
        }

        Ok(())
    }

    async fn load_ctx(
        &self,
        path: std::path::PathBuf,
        sys_prefix: Arc<str>,
        ctx: Arc<str>,
    ) -> Result<()> {
        let mut dir = tokio::fs::read_dir(&path).await?;
        while let Some(e) = dir.next_entry().await? {
            if e.file_type().await?.is_dir() {
                self.load_h1(e.path(), sys_prefix.clone(), ctx.clone())
                    .await?;
            }
        }

        Ok(())
    }

    async fn load_h1(
        &self,
        path: std::path::PathBuf,
        sys_prefix: Arc<str>,
        ctx: Arc<str>,
    ) -> Result<()> {
        let mut dir = tokio::fs::read_dir(&path).await?;
        while let Some(e) = dir.next_entry().await? {
            if e.file_type().await?.is_dir() {
                self.load_h2(e.path(), sys_prefix.clone(), ctx.clone())
                    .await?;
            }
        }

        Ok(())
    }

    async fn load_h2(
        &self,
        path: std::path::PathBuf,
        sys_prefix: Arc<str>,
        ctx: Arc<str>,
    ) -> Result<()> {
        let mut dir = tokio::fs::read_dir(&path).await?;
        while let Some(e) = dir.next_entry().await? {
            if e.file_type().await?.is_file() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("meta-") {
                    let hash = name.trim_start_matches("meta-");
                    self.load_meta(
                        e.path(),
                        path.join(format!("data-{hash}")),
                        sys_prefix.clone(),
                        ctx.clone(),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn load_meta(
        &self,
        meta_path: std::path::PathBuf,
        data_path: std::path::PathBuf,
        sys_prefix: Arc<str>,
        ctx: Arc<str>,
    ) -> Result<()> {
        let meta: Arc<str> = tokio::fs::read_to_string(&meta_path)
            .await?
            .trim()
            .to_string()
            .into();
        let meta = ObjMeta(meta);
        if meta.sys_prefix() != &*sys_prefix || meta.ctx() != &*ctx {
            tracing::warn!(?meta_path, "corrupt obj store on disk");
            return Ok(());
        }
        if !tokio::fs::metadata(&data_path).await?.is_file() {
            tracing::warn!(?data_path, "corrupt obj store on disk");
            return Ok(());
        }

        let path_list = {
            let mut lock = self.index.lock().unwrap();
            lock.put(
                meta,
                Info {
                    meta_path,
                    data_path,
                },
            );
            lock.get_delete()
        };

        destroy(path_list).await;

        Ok(())
    }
}

impl Obj for ObjFile {
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<(Arc<str>, Bytes)>> {
        Box::pin(async move {
            let (meta, info) = self.index.lock().unwrap().get(ObjMeta(path))?;
            let data = tokio::fs::read(info.data_path).await?.into();
            Ok((meta.0, data))
        })
    }

    fn rm(&self, path: Arc<str>) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let path_list = {
                let mut lock = self.index.lock().unwrap();
                lock.rm(ObjMeta(path));
                lock.get_delete()
            };

            destroy(path_list).await;
            Ok(())
        })
    }

    fn list(
        &self,
        path_prefix: Arc<str>,
        created_gt: f64,
        limit: u32,
    ) -> BoxFut<'_, Result<Vec<Arc<str>>>> {
        Box::pin(async move {
            Ok(self
                .index
                .lock()
                .unwrap()
                .list(path_prefix, created_gt, limit))
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
            safe_str(meta.app_path())?;
            if meta.app_path().is_empty() {
                return Err(Error::other("appPath cannot be empty"));
            }

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
            let path_list = {
                let mut lock = self.index.lock().unwrap();
                lock.put(
                    meta,
                    Info {
                        meta_path,
                        data_path,
                    },
                );
                lock.get_delete()
            };

            destroy(path_list).await;

            Ok(())
        })
    }
}

async fn destroy(list: Vec<(ObjMeta, Info)>) {
    for (
        _,
        Info {
            meta_path,
            data_path,
        },
    ) in list
    {
        if let Err(err) = tokio::fs::remove_file(&meta_path).await {
            tracing::warn!(?err, "failed to remove object store path");
        }
        if let Err(err) = tokio::fs::remove_file(&data_path).await {
            tracing::warn!(?err, "failed to remove object store path");
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

        let mut list = of.list("c/AAAA/b".into(), 0.0, 1).await.unwrap();
        assert_eq!(1, list.len());

        let item = list.remove(0);
        assert_eq!("c/AAAA/bob/1.0/0.0", &*item);

        let got = of.get("c/AAAA/bob/1.0/0.0".into()).await.unwrap().1;
        assert_eq!(&b"hello"[..], &got[..]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn clean_replaced_items() {
        let td = tempfile::tempdir().unwrap();

        let of = ObjFile::create(Some(td.path().into())).await.unwrap();

        of.put(
            "c/AAAA/bob/1.0/0.0".into(),
            bytes::Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        of.put(
            "c/AAAA/bob/2.0/0.0".into(),
            bytes::Bytes::from_static(b"world"),
        )
        .await
        .unwrap();

        let mut file_count = 0;

        let mut dir = async_walkdir::WalkDir::new(td.path());
        use futures::StreamExt;
        while let Some(entry) = dir.next().await {
            let entry = entry.unwrap();
            if entry.path().is_file()
                && entry.file_name().to_string_lossy().starts_with("meta-")
            {
                println!("{:?}", entry.path());
                file_count += 1;
            }
        }

        assert_eq!(1, file_count);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_unknown_time() {
        let of = ObjFile::create(None).await.unwrap();

        of.put(
            "c/AAAA/bob/1.0/0.0".into(),
            bytes::Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        of.put(
            "c/AAAA/ned/2.0/0.0".into(),
            bytes::Bytes::from_static(b"world"),
        )
        .await
        .unwrap();

        let got = of.get("c/AAAA/bob/0.0/0.0".into()).await.unwrap().1;
        assert_eq!(&b"hello"[..], &got[..]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load() {
        let tmp = tempfile::tempdir().unwrap();

        let of1 = ObjFile::create(Some(tmp.path().into())).await.unwrap();

        of1.put(
            "c/AAAA/bob/1.0/0.0".into(),
            bytes::Bytes::from_static(b"hello"),
        )
        .await
        .unwrap();

        drop(of1);

        let of2 = ObjFile::create(Some(tmp.path().into())).await.unwrap();

        let got = of2.get("c/AAAA/bob/1.0/0.0".into()).await.unwrap().1;
        assert_eq!(&b"hello"[..], &got[..]);
    }
}
