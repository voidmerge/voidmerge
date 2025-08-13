//! VoidMerge types.

use crate::*;

/// A boxed future.
pub type BoxFut<'lt, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'lt + Send>>;

/// Wrapper error type.
#[derive(Clone, Debug)]
pub struct WithInfo {
    /// The inner source error.
    pub error: Arc<std::io::Error>,

    /// The additional info.
    pub info: Arc<str>,
}

impl std::fmt::Display for WithInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)?;
        f.write_str(": ")?;
        f.write_str(&self.info)
    }
}

impl std::error::Error for WithInfo {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl WithInfo {
    /// Construct a new WithInfo error type.
    pub fn new(
        error: impl Into<Arc<std::io::Error>>,
        info: impl Into<String>,
    ) -> Self {
        Self {
            error: error.into(),
            info: info.into().into(),
        }
    }
}

/// Extension trait for std::io::Error.
pub trait ErrorExt {
    /// Wrap this error with some additional context information.
    fn with_info(self, info: String) -> std::io::Error;
}

impl ErrorExt for std::io::Error {
    fn with_info(self, info: String) -> std::io::Error {
        std::io::Error::new(self.kind(), WithInfo::new(self, info))
    }
}

impl ErrorExt for std::io::ErrorKind {
    fn with_info(self, info: String) -> std::io::Error {
        std::io::Error::from(self).with_info(info)
    }
}

mod value;
pub use value::*;

mod hash;
pub use hash::*;

mod encoding;
pub use encoding::*;

mod module_context_store;
pub use module_context_store::*;

mod module_logic;
pub use module_logic::*;

mod module_runtime_store;
pub use module_runtime_store::*;

mod module_sign;
pub use module_sign::*;
