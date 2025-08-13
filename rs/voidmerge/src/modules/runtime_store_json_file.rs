use crate::*;
use std::collections::HashMap;
use types::*;

/// A json file backed runtime store.
#[derive(Debug)]
pub struct RuntimeStoreJsonFileFactory;

impl ModuleRuntimeStoreFactory for RuntimeStoreJsonFileFactory {
    fn factory(
        &self,
        config: Arc<config::Config>,
    ) -> BoxFut<'static, Result<DynModuleRuntimeStore>> {
        Box::pin(async move { RuntimeStoreJsonFile::create(config).await })
    }
}

struct RuntimeStoreJsonFile {
    cache: Mutex<HashMap<Arc<str>, Arc<str>>>,
    lock: Arc<Mutex<fd_lock::RwLock<std::fs::File>>>,
}

impl std::fmt::Debug for RuntimeStoreJsonFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleRuntimeStoreJsonFile").finish()
    }
}

impl RuntimeStoreJsonFile {
    pub async fn create(
        config: Arc<config::Config>,
    ) -> Result<DynModuleRuntimeStore> {
        tokio::fs::create_dir_all(&config.data_dir).await?;
        let runtime_name = config.data_dir.join("runtime.json");
        let lock = tokio::task::spawn_blocking(move || {
            std::io::Result::Ok(fd_lock::RwLock::new(
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(runtime_name)?,
            ))
        })
        .await??;
        let this = Arc::new(Self {
            cache: Default::default(),
            lock: Arc::new(Mutex::new(lock)),
        });
        this.load().await?;
        Ok(this)
    }

    async fn load(&self) -> Result<()> {
        let lock = self.lock.clone();
        let data = tokio::task::spawn_blocking(move || {
            use std::io::{Read, Seek};
            let mut lock = lock.lock().unwrap();
            let mut lock = lock.write()?;
            lock.rewind()?;
            let mut out = Vec::new();
            lock.read_to_end(&mut out)?;
            std::io::Result::Ok(out)
        })
        .await??;

        let data: HashMap<Arc<str>, Arc<str>> = match serde_json::from_slice(
            &data,
        )
        .map_err(std::io::Error::other)
        {
            Err(_) => return Ok(()),
            Ok(data) => data,
        };

        *self.cache.lock().unwrap() = data;
        Ok(())
    }

    async fn store(&self, data: String) -> Result<()> {
        let lock = self.lock.clone();
        tokio::task::spawn_blocking(move || {
            // TODO - write to tmpfile first, then swap?
            use std::io::{Seek, Write};
            let mut lock = lock.lock().unwrap();
            let mut lock = lock.write()?;
            lock.rewind()?;
            lock.write_all(data.as_bytes())?;
            std::io::Result::Ok(())
        })
        .await?
    }
}

impl ModuleRuntimeStore for RuntimeStoreJsonFile {
    fn set<'a, 'b: 'a>(
        &'a self,
        k: Arc<str>,
        edit: RuntimeStoreAtomicEdit<'b>,
    ) -> BoxFut<'a, Result<(bool, Option<Arc<str>>)>> {
        Box::pin(async move {
            let (data, next) = {
                let mut lock = self.cache.lock().unwrap();

                let cur = lock.get(&k).cloned();
                let next = match edit(cur.clone())? {
                    None => return Ok((false, cur)),
                    Some(next) => next,
                };

                lock.insert(k, next.clone());

                (serde_json::to_string_pretty(&*lock)?, next)
            };

            self.store(data).await?;

            Ok((true, Some(next)))
        })
    }

    fn list(&self) -> Vec<Arc<str>> {
        self.cache.lock().unwrap().keys().cloned().collect()
    }

    fn get(&self, k: &str) -> Option<Arc<str>> {
        self.cache.lock().unwrap().get(k).cloned()
    }
}
