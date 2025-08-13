//! VoidMerge configuration types.

use crate::types::*;
use crate::*;

/// VoidMerge runtime configuration.
#[derive(Debug)]
pub struct Config {
    /// The context store factory.
    pub context_store: DynModuleContextStoreFactory,

    /// The runtime store factory.
    pub runtime_store: DynModuleRuntimeStoreFactory,

    /// The logic factori.
    pub logic: DynModuleLogicFactory,

    /// The configured signature modules.
    pub sign: Vec<DynModuleSign>,

    /// The list of sysadmin tokens to accept.
    pub sysadmin_tokens: Vec<String>,

    /// Adds a redirect at "/" to "/web/{default_context}/index.html".
    pub default_context: Option<Hash>,

    /// The http_addr to bind.
    pub http_addr: String,

    /// Where to store runtime data.
    pub data_dir: std::path::PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let mut sign: Vec<DynModuleSign> = Vec::new();

        // this just prevent mut warnings on feature diffs
        sign.clear();

        #[cfg(feature = "p256")]
        sign.push(Arc::new(modules::SignP256));

        Self {
            context_store: Arc::new(modules::ContextStoreFileFactory),
            runtime_store: Arc::new(modules::RuntimeStoreJsonFileFactory),

            #[cfg(not(feature = "v8"))]
            logic: Arc::new(modules::LogicFactoryStub),

            #[cfg(feature = "v8")]
            logic: Arc::new(modules::LogicFactoryV8),

            sign,

            sysadmin_tokens: Vec::default(),
            default_context: None,
            http_addr: "[::]:8080".to_string(),
            data_dir: ".".into(),
        }
    }
}
