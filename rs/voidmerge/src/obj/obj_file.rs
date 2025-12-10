//! File-backed object store.

use crate::obj::*;
use std::collections::{BTreeMap, HashSet};
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
                        let path_list = this.inner.lock().unwrap().prune();
                        for path in path_list {
                            destroy(path).await;
                        }

                        let now = std::time::Instant::now();
                        if now - last_meter > std::time::Duration::from_secs(60)
                        {
                            last_meter = now;
                            let map = this.inner.lock().unwrap().meter();
                            for (ctx, storage) in map {
                                crate::meter::meter_storage_gib(
                                    &ctx,
                                    storage as f64 / 1073741824.0,
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
                inner: Mutex::new(Inner::new()),
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

        let d = self.inner.lock().unwrap().put(meta, meta_path, data_path);

        if let Some((d1, d2)) = d {
            destroy(d1).await;
            destroy(d2).await;
        }

        Ok(())
    }
}

impl Obj for ObjFile {
    fn get(&self, path: Arc<str>) -> BoxFut<'_, Result<(Arc<str>, Bytes)>> {
        Box::pin(async move {
            let (meta, data_path) =
                self.inner.lock().unwrap().get(ObjMeta(path))?;
            let data = tokio::fs::read(data_path).await?.into();
            Ok((meta.0, data))
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
                .inner
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
            let d = self.inner.lock().unwrap().put(meta, meta_path, data_path);

            if let Some((d1, d2)) = d {
                destroy(d1).await;
                destroy(d2).await;
            }

            Ok(())
        })
    }
}

async fn destroy(path: std::path::PathBuf) {
    if let Err(err) = tokio::fs::remove_file(&path).await {
        tracing::warn!(?err, "failed to remove object store path");
    }
}

struct Item {
    pub meta: ObjMeta,
    pub meta_path: std::path::PathBuf,
    pub data_path: std::path::PathBuf,
}

struct Inner(OrderMap<Item>);

impl Inner {
    pub fn new() -> Self {
        Self(OrderMap::default())
    }

    pub fn meter(&self) -> HashMap<Arc<str>, u64> {
        let mut map: HashMap<Arc<str>, u64> = Default::default();
        for Item { meta, .. } in self.0.iter(f64::MIN, f64::MAX) {
            if meta.sys_prefix() != ObjMeta::SYS_CTX {
                continue;
            }
            *map.entry(meta.ctx().into()).or_default() += meta.byte_length();
        }
        map
    }

    pub fn prune(&mut self) -> Vec<std::path::PathBuf> {
        let now = safe_now();
        let mut destroy = Vec::new();
        self.0.retain(|_, v| {
            let x = v.meta.expires_secs();
            if x == 0.0 || x > now {
                true
            } else {
                destroy.push(v.meta_path.clone());
                destroy.push(v.data_path.clone());
                false
            }
        });
        destroy
    }

    pub fn get(&self, meta: ObjMeta) -> Result<(ObjMeta, std::path::PathBuf)> {
        let pfx = Pfx::new(&meta);
        self.0
            .get(&pfx)
            .map(|item| (item.meta.clone(), item.data_path.clone()))
            .ok_or_else(|| {
                Error::not_found(format!("Could not locate meta: {meta}"))
            })
    }

    pub fn list(
        &self,
        prefix: Arc<str>,
        created_gt: f64,
        limit: u32,
    ) -> Vec<Arc<str>> {
        let mut out = Vec::new();
        let mut last_created_secs = 0.0;
        for item in self.0.iter(created_gt, f64::MAX) {
            let created_secs = item.meta.created_secs();
            if out.len() >= limit as usize && created_secs > last_created_secs {
                // in edge case of exactly matching created_secs, we may return
                // more than the limit, but if we don't do this, the continue
                // token will cause them to miss some items
                return out;
            }
            last_created_secs = created_secs;
            if created_secs > created_gt && item.meta.0.starts_with(&*prefix) {
                out.push(item.meta.0.clone());
            }
        }
        out
    }

    pub fn put(
        &mut self,
        meta: ObjMeta,
        meta_path: std::path::PathBuf,
        data_path: std::path::PathBuf,
    ) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        let now = safe_now();
        let mx = meta.expires_secs();
        if mx > 0.0 && mx < now {
            return Some((meta_path, data_path));
        }
        let pfx = Pfx::new(&meta);
        let item = Item {
            meta,
            meta_path,
            data_path,
        };
        let created_secs = item.meta.created_secs();
        if let Some(orig_item) = self.0.insert(created_secs, pfx, item) {
            let ox = orig_item.meta.expires_secs();
            if ox > 0.0 && ox < now {
                return Some((orig_item.meta_path, orig_item.data_path));
            }
            let orig_created_secs = orig_item.meta.created_secs();
            if orig_created_secs >= created_secs {
                // woops, put it back
                if let Some(item) = self.0.insert(
                    orig_created_secs,
                    Pfx::new(&orig_item.meta),
                    orig_item,
                ) {
                    return Some((item.meta_path, item.data_path));
                }
            } else {
                return Some((orig_item.meta_path, orig_item.data_path));
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
struct Order(f64);

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Order {}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Pfx(Arc<str>);

impl Pfx {
    pub fn new(meta: &ObjMeta) -> Self {
        Self(format!(
            "{}/{}/{}",
            meta.sys_prefix(),
            meta.ctx(),
            meta.app_path(),
        ).into())
    }
}

struct OrderMap<T> {
    map: HashMap<Pfx, (Order, T)>,
    order: BTreeMap<Order, HashSet<Pfx>>,
}

impl<T> Default for OrderMap<T> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            order: Default::default(),
        }
    }
}

impl<T> OrderMap<T> {
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&Pfx, &T) -> bool,
    {
        let mut remove = Vec::new();
        for (pfx, (_, t)) in self.map.iter() {
            if !f(pfx, t) {
                remove.push(pfx.clone());
            }
        }
        for pfx in remove {
            self.remove(&pfx);
        }
    }

    pub fn remove(&mut self, pfx: &Pfx) -> Option<T> {
        if let Some((order, t)) = self.map.remove(pfx) {
            let mut remove = false;
            if let Some(set) = self.order.get_mut(&order) {
                set.remove(pfx);
                if set.is_empty() {
                    remove = true;
                }
            }
            if remove {
                self.order.remove(&order);
            }
            Some(t)
        } else {
            None
        }
    }

    pub fn insert(&mut self, order: f64, pfx: Pfx, val: T) -> Option<T> {
        let out = self.remove(&pfx);
        let order = Order(order);
        self.map.insert(pfx.clone(), (order, val));
        self.order.entry(order).or_default().insert(pfx);
        out
    }

    pub fn get(&self, pfx: &Pfx) -> Option<&T> {
        self.map.get(pfx).map(|v| &v.1)
    }

    pub fn iter(&self, start: f64, end: f64) -> impl Iterator<Item = &T> {
        self.order
            .range(Order(start)..Order(end))
            .flat_map(|(_, set)| {
                set.iter().filter_map(|pfx| self.map.get(pfx).map(|v| &v.1))
            })
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
