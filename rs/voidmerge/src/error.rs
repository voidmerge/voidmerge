//! VoidMerge error types.

use std::error::Error as StdError;
pub use std::io::{Error, Result};
use std::sync::Arc;

/// Convenience extension trait helpers for error types.
pub trait ErrorExt: Send + Sync {
    /// Convert to a clonable type.
    fn into_sync(self) -> Arc<dyn StdError + Send + Sync>;

    /// Add additional information to the error.
    fn with_info(
        self,
        info: impl Into<Box<dyn StdError + Send + Sync>>,
    ) -> Error;

    /// An error indicating an operation took too long.
    fn timeout(src: impl Into<Box<dyn StdError + Send + Sync>>) -> Error;
}

impl ErrorExt for Error {
    fn into_sync(self) -> Arc<dyn StdError + Send + Sync> {
        let out: Box<dyn StdError + Send + Sync> = self.into();
        out.into()
    }

    fn with_info(
        self,
        info: impl Into<Box<dyn StdError + Send + Sync>>,
    ) -> Error {
        let kind = self.kind();

        #[derive(Debug)]
        struct Err(
            pub Box<dyn StdError + Send + Sync>,
            pub Box<dyn StdError + Send + Sync>,
        );

        impl std::fmt::Display for Err {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)?;
                f.write_str(": ")?;
                self.1.fmt(f)
            }
        }

        impl StdError for Err {}

        let err = Err(
            self.into_inner()
                .map(Into::into)
                .unwrap_or_else(|| "none".into()),
            info.into(),
        );

        std::io::Error::new(kind, err)
    }

    fn timeout(src: impl Into<Box<dyn StdError + Send + Sync>>) -> Error {
        std::io::Error::new(std::io::ErrorKind::TimedOut, src)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn error_fmt() {
        eprintln!("{}", Error::timeout("test1").with_info("hello"));
        eprintln!("{:?}", Error::timeout("test2").with_info("world"));
    }
}
