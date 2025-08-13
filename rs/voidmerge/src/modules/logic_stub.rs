//! A stub [ModuleLogic] module.

use crate::*;
use types::*;

/// A stub [ModuleLogic] module.
#[derive(Debug)]
pub struct LogicFactoryStub;

impl ModuleLogicFactory for LogicFactoryStub {
    fn factory(
        &self,
        _config: Arc<config::Config>,
    ) -> BoxFut<'static, Result<DynModuleLogic>> {
        Box::pin(async move {
            let out: DynModuleLogic = Arc::new(LogicStub);
            Ok(out)
        })
    }
}

struct LogicStub;

impl ModuleLogic for LogicStub {
    fn default_logic(&self) -> VmLogic {
        VmLogic::Utf8Single { code: "".into() }
    }

    fn exec(&self, exec: ModuleLogicExec) -> BoxFut<'_, Result<Value>> {
        Box::pin(async move {
            // just return the input
            Ok(exec.input)
        })
    }
}
