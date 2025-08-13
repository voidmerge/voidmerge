use crate::*;
use std::collections::HashMap;
use types::*;

/// A store that writes files to disk.
#[derive(Debug)]
pub struct ContextStoreFileFactory;

impl ModuleContextStoreFactory for ContextStoreFileFactory {
    fn factory(
        &self,
        runtime: Arc<runtime::Runtime>,
        context: Hash,
    ) -> BoxFut<'static, Result<DynModuleContextStore>> {
        Box::pin(async move {
            let dir = runtime
                .config()
                .data_dir
                .join("store")
                .join(context.to_string());
            let out: DynModuleContextStore =
                Arc::new(ContextStoreFile::new(dir).await?);
            Ok(out)
        })
    }
}

mod str_hash {
    use super::*;

    pub fn serialize<S>(t: &Hash, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(&t.to_string())
    }

    pub fn deserialize<'de, D>(d: D) -> std::result::Result<Hash, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = serde::Deserialize::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

type Type = Arc<str>;
type Short = Hash;
type Ident = Hash;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexItem {
    #[serde(rename = "type")]
    type_: Arc<str>,
    #[serde(with = "str_hash")]
    ident: Ident,
    #[serde(with = "str_hash")]
    short: Short,
    size: f64,
    ttl_s: Option<f64>,
}

#[derive(Debug)]
struct ContextStoreFile {
    index: Mutex<HashMap<(Type, Ident), IndexItem>>,
    dir: std::path::PathBuf,
}

impl ContextStoreFile {
    async fn new(dir: std::path::PathBuf) -> Result<Self> {
        let this = Self {
            index: Default::default(),
            dir,
        };

        this.load_from_disk()
            .await
            .map_err(|e| e.with_info("loading index from disk".into()))?;

        Ok(this)
    }

    fn path_root(&self, type_: &str, ident: &Hash) -> std::path::PathBuf {
        #[inline(always)]
        fn get(ident: &Hash, idx: usize) -> String {
            format!(
                "h{:02x}{:02x}",
                ident.get(idx).unwrap_or(&0),
                ident.get(idx + 1).unwrap_or(&0)
            )
        }
        let a = get(ident, 0);
        let b = get(ident, 2);
        let c = get(ident, 4);
        self.dir
            .join(type_)
            .join(a)
            .join(b)
            .join(c)
            .join(ident.to_string())
    }

    fn check_cur(
        lock: &HashMap<(Type, Ident), IndexItem>,
        type_: Arc<str>,
        ident: Ident,
        cur: &Option<Arc<VmObjSigned>>,
    ) -> Result<Option<Short>> {
        let idx = lock.get(&(type_, ident));
        match (idx, cur) {
            (None, None) => return Ok(None),
            (
                Some(IndexItem {
                    type_,
                    ident,
                    short,
                    ..
                }),
                Some(cur),
            ) => {
                if *type_ == cur.type_ && *ident == cur.canon_ident() {
                    return Ok(Some(short.clone()));
                }
            }
            _ => (),
        }
        Err(std::io::Error::other("mismatching previous item"))
    }

    async fn load_from_disk(&self) -> Result<()> {
        let now = std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .as_secs_f64();

        tokio::fs::create_dir_all(&self.dir).await?;
        let mut d1 = tokio::fs::read_dir(&self.dir).await?;
        while let Some(e1) = d1.next_entry().await? {
            if !e1.file_type().await?.is_dir() {
                continue;
            }
            let type_ = e1.file_name().to_string_lossy().to_string();

            let mut d2 = tokio::fs::read_dir(e1.path()).await?;
            while let Some(e2) = d2.next_entry().await? {
                if !e2.file_type().await?.is_dir() {
                    continue;
                }

                let mut d3 = tokio::fs::read_dir(e2.path()).await?;
                while let Some(e3) = d3.next_entry().await? {
                    if !e3.file_type().await?.is_dir() {
                        continue;
                    }

                    let mut d4 = tokio::fs::read_dir(e3.path()).await?;
                    while let Some(e4) = d4.next_entry().await? {
                        if !e4.file_type().await?.is_dir() {
                            continue;
                        }

                        let mut d5 = tokio::fs::read_dir(e4.path()).await?;
                        while let Some(e5) = d5.next_entry().await? {
                            if !e5.file_type().await?.is_dir() {
                                continue;
                            }
                            let ident: Hash =
                                e5.file_name().to_string_lossy().parse()?;

                            let path_root = self.path_root(&type_, &ident);

                            let _g = PathLockGuard::new(&path_root).await?;

                            let fn_cur = e5.path().join("cur");

                            let index =
                                tokio::fs::read_to_string(fn_cur).await?;
                            let index: IndexItem =
                                serde_json::from_str(&index)?;

                            if let Some(ttl_s) = &index.ttl_s {
                                if *ttl_s < now {
                                    continue;
                                    // TODO cleanup the disk
                                }
                            }

                            self.index.lock().unwrap().insert(
                                (index.type_.clone(), index.ident.clone()),
                                index,
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

struct PathLockGuard {
    done_s: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for PathLockGuard {
    fn drop(&mut self) {
        std::mem::drop(self.done_s.take());
        self.task.abort();
    }
}

impl PathLockGuard {
    pub async fn new(path_root: &std::path::Path) -> Result<Self> {
        let fn_lock = path_root.join("lock");

        let (ready_s, ready_r) = tokio::sync::oneshot::channel();
        let (done_s, done_r) = tokio::sync::oneshot::channel();
        let task = tokio::task::spawn_blocking(move || {
            let file = match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(fn_lock)
            {
                Ok(file) => file,
                Err(err) => {
                    let _ = ready_s.send(Err(err));
                    return;
                }
            };

            let mut lock = fd_lock::RwLock::new(file);

            let _lock = match lock.write() {
                Ok(lock) => lock,
                Err(err) => {
                    let _ = ready_s.send(Err(err));
                    return;
                }
            };

            if ready_s.send(Ok(())).is_err() {
                return;
            }

            let _ = done_r.blocking_recv();
        });

        ready_r.await.map_err(|_| {
            std::io::Error::from(std::io::ErrorKind::BrokenPipe)
        })??;

        Ok(Self {
            done_s: Some(done_s),
            task,
        })
    }
}

impl ModuleContextStore for ContextStoreFile {
    fn insert(
        &self,
        cur: Option<Arc<VmObjSigned>>,
        data: Arc<VmObjSigned>,
    ) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let ident = data.canon_ident();

            Self::check_cur(
                &self.index.lock().unwrap(),
                data.type_.clone(),
                ident.clone(),
                &cur,
            )?;

            let path_root = self.path_root(&data.type_, &ident);

            tokio::fs::create_dir_all(&path_root).await?;

            let _g = PathLockGuard::new(&path_root).await?;

            let fn_short = path_root.join(data.short().to_string());
            let fn_prev = path_root.join("prev");
            let fn_cur = path_root.join("cur");
            let fn_next = path_root.join("next");

            let enc = encode(&data)?;

            let _ = tokio::fs::remove_file(&fn_short).await;
            tokio::fs::write(fn_short, &enc).await?;

            let idx = IndexItem {
                type_: data.type_.clone(),
                ident: ident.clone(),
                short: data.short(),
                size: data.enc.len() as f64,
                ttl_s: data.ttl_s,
            };

            let idx_enc = serde_json::to_string_pretty(&idx)?;

            let _ = tokio::fs::remove_file(&fn_prev).await;
            let _ = tokio::fs::remove_file(&fn_next).await;

            tokio::fs::write(&fn_next, &idx_enc).await?;

            let old_short = Self::check_cur(
                &self.index.lock().unwrap(),
                data.type_.clone(),
                ident.clone(),
                &cur,
            )?;

            let _ = tokio::fs::rename(&fn_cur, &fn_prev).await;
            tokio::fs::rename(&fn_next, &fn_cur).await?;

            self.index
                .lock()
                .unwrap()
                .insert((data.type_.clone(), ident), idx);

            if let Some(old_short) = old_short {
                if old_short != data.short() {
                    let fn_old_short = path_root.join(old_short.to_string());
                    let _ = tokio::fs::remove_file(fn_old_short).await;
                }
            }

            Ok(())
        })
    }

    fn select(&self, select: VmSelect) -> BoxFut<'_, Result<VmSelectResponse>> {
        Box::pin(async move {
            let mut count = 0.0;
            let mut size = 0.0;

            let now = std::time::SystemTime::UNIX_EPOCH
                .elapsed()
                .unwrap()
                .as_secs_f64();

            let mut results: Vec<VmSelectResponseItem> = Vec::new();

            self.index.lock().unwrap().retain(|_, item| {
                if let Some(ttl_s) = &item.ttl_s {
                    if *ttl_s < now {
                        return false;
                        // TODO delete off disk
                    }
                }

                if let Some(filter) = &select.filter_by_types {
                    if !filter.contains(&item.type_) {
                        return true;
                    }
                }
                if let Some(filter) = &select.filter_by_idents {
                    if !filter.contains(&item.ident) {
                        return true;
                    }
                }
                if let Some(filter) = &select.filter_by_shorts {
                    if !filter.contains(&item.short) {
                        return true;
                    }
                }

                count += 1.0;
                size += item.size;

                let mut out_item = VmSelectResponseItem {
                    ident: Some(item.ident.clone()),
                    type_: Some(item.type_.clone()),
                    short: Some(item.short.clone()),
                    ..Default::default()
                };

                if matches!(select.return_size, Some(true)) {
                    out_item.size = Some(item.size);
                }

                results.push(out_item);

                true
            });

            if matches!(select.return_data, Some(true)) {
                for res in results.iter_mut() {
                    let path_root = self.path_root(
                        res.type_.as_ref().unwrap(),
                        res.ident.as_ref().unwrap(),
                    );

                    tokio::fs::create_dir_all(&path_root).await?;

                    let _g = PathLockGuard::new(&path_root).await?;

                    let short_path =
                        path_root.join(res.short.as_ref().unwrap().to_string());

                    let data: Arc<VmObjSigned> =
                        decode(&tokio::fs::read(short_path).await?)?;

                    if data.short() != *res.short.as_ref().unwrap() {
                        return Err(std::io::Error::other("disk corrupted"));
                    }

                    res.data = Some(data);

                    if !matches!(select.return_ident, Some(true)) {
                        res.ident = None;
                    }
                    if !matches!(select.return_type, Some(true)) {
                        res.type_ = None;
                    }
                    if !matches!(select.return_short, Some(true)) {
                        res.short = None;
                    }
                }
            }

            Ok(VmSelectResponse {
                count,
                size,
                results,
            })
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn obj(
        sign: &MultiSign,
        type_: Arc<str>,
        ident: Option<Hash>,
    ) -> Arc<VmObjSigned> {
        VmObj {
            type_,
            ident,
            deps: None,
            ttl_s: None,
            app: Some(Hash::nonce().to_string().into()),
        }
        .sign(sign)
        .unwrap()
        .into()
    }

    #[tokio::test]
    async fn load_from_disk() -> std::io::Result<()> {
        let dir = tempfile::tempdir().unwrap();

        let runtime = runtime::Runtime::new(Arc::new(config::Config {
            data_dir: dir.path().into(),
            ..Default::default()
        }))
        .await?;

        let item1 = obj(runtime.sign(), "test".into(), None);
        let item2 = obj(runtime.sign(), "test".into(), None);

        let store1 = ContextStoreFile::new(dir.path().into()).await.unwrap();
        store1.insert(None, item1.clone()).await.unwrap();
        store1.insert(None, item2.clone()).await.unwrap();

        std::mem::drop(store1);

        let store2 = ContextStoreFile::new(dir.path().into()).await.unwrap();

        let res = store2
            .select(VmSelect {
                return_data: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        let mut found_count = 0;

        for res in res.results {
            let data = res.data.unwrap();
            if data != item1 && data != item2 {
                panic!("unexpected item");
            }
            found_count += 1;
        }

        if found_count != 2 {
            panic!("did not find all items");
        }

        Ok(())
    }

    #[tokio::test]
    async fn same_ident() -> std::io::Result<()> {
        let id: Hash = Hash::nonce();

        let dir = tempfile::tempdir().unwrap();

        let runtime = runtime::Runtime::new(Arc::new(config::Config {
            data_dir: dir.path().into(),
            ..Default::default()
        }))
        .await?;

        let store = ContextStoreFile::new(dir.path().into()).await.unwrap();

        let item1 = obj(runtime.sign(), "test".into(), Some(id.clone()));
        store.insert(None, item1.clone()).await.unwrap();

        let res1 = store
            .select(VmSelect {
                return_data: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(
            vec![Some(item1.clone())],
            res1.results.into_iter().map(|r| r.data).collect::<Vec<_>>()
        );

        let item2 = obj(runtime.sign(), "test".into(), Some(id));
        store.insert(Some(item1), item2.clone()).await.unwrap();

        let res2 = store
            .select(VmSelect {
                return_data: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(
            vec![Some(item2.clone())],
            res2.results.into_iter().map(|r| r.data).collect::<Vec<_>>()
        );

        Ok(())
    }
}
