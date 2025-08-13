//! VoidMerge runtime.

use crate::*;
use types::*;

/// VoidMerge runtime.
#[derive(Debug)]
pub struct Runtime {
    config: Arc<config::Config>,
    runtime_store: DynModuleRuntimeStore,
    sign: Arc<MultiSign>,
}

impl Runtime {
    /// Construct a new VoidMerge [Runtime].
    pub async fn new(config: Arc<config::Config>) -> Result<Arc<Self>> {
        let runtime_store =
            config.runtime_store.factory(config.clone()).await?;

        let sign = Arc::new(MultiSign::new(runtime_store.clone()));
        for sm in config.sign.iter() {
            sign.add_sign(sm.clone()).await?;
        }

        Ok(Arc::new(Self {
            config,
            runtime_store,
            sign,
        }))
    }

    /// Config.
    pub fn config(&self) -> &Arc<config::Config> {
        &self.config
    }

    /// Runtime store.
    pub fn runtime_store(&self) -> &DynModuleRuntimeStore {
        &self.runtime_store
    }

    /// Get the multisigner.
    pub fn sign(&self) -> &Arc<MultiSign> {
        &self.sign
    }
}
