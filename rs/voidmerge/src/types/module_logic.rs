use super::*;

/// Factory for a [ModuleLogic] module.
pub trait ModuleLogicFactory: std::fmt::Debug + 'static + Send + Sync {
    /// Create the [ModuleLogic] module.
    fn factory(
        &self,
        config: Arc<config::Config>,
    ) -> BoxFut<'static, Result<DynModuleLogic>>;
}

/// Trait object [ModuleLogicFactory].
pub type DynModuleLogicFactory =
    Arc<dyn ModuleLogicFactory + 'static + Send + Sync>;

/// Defines a module that can perform arbitrary logic execution.
pub trait ModuleLogic: 'static + Send + Sync {
    /// Get the default permissive logic.
    fn default_logic(&self) -> VmLogic;

    /// Execute the logic.
    fn exec(&self, exec: ModuleLogicExec) -> BoxFut<'_, Result<Value>>;
}

/// Trait object [ModuleLogic].
pub type DynModuleLogic = Arc<dyn ModuleLogic + 'static + Send + Sync>;

/// Closure type for system-defined callback logic function.
pub type ModuleLogicSystemCb =
    Arc<dyn Fn(Value) -> Result<Value> + 'static + Send + Sync>;

/// The input data required to process a logic execution.
pub struct ModuleLogicExec {
    /// The code that will be executed.
    pub logic: VmLogic,

    /// Callback logic for system functions.
    pub system_cb: ModuleLogicSystemCb,

    /// The input data for the function.
    pub input: Value,

    /// Execution timeout.
    pub timeout: Option<std::time::Duration>,
}
