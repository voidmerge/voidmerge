use super::*;

/// Factory for a [ModuleContextStore] module.
pub trait ModuleContextStoreFactory:
    std::fmt::Debug + 'static + Send + Sync
{
    /// Create the [ModuleContextStore] module.
    fn factory(
        &self,
        runtime: Arc<runtime::Runtime>,
        context: Hash,
    ) -> BoxFut<'static, Result<DynModuleContextStore>>;
}

/// Trait object [ModuleContextStoreFactory].
pub type DynModuleContextStoreFactory =
    Arc<dyn ModuleContextStoreFactory + 'static + Send + Sync>;

/// Defines a module that is a runtime KV store.
pub trait ModuleContextStore: std::fmt::Debug + 'static + Send + Sync {
    /// Put an item into the context store.
    ///
    /// If "cur" is_some, this will fail if the current item in the database
    /// does not equal the item passed in, to preserve atomicity.
    fn insert(
        &self,
        cur: Option<Arc<VmObjSigned>>,
        data: Arc<VmObjSigned>,
    ) -> BoxFut<'_, Result<()>>;

    /// Query data in the store.
    fn select(&self, select: VmSelect) -> BoxFut<'_, Result<VmSelectResponse>>;
}

/// Trait object [ModuleContextStore].
pub type DynModuleContextStore =
    Arc<dyn ModuleContextStore + 'static + Send + Sync>;
