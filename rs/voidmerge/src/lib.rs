#![deny(missing_docs)]
//! VoidMerge: P2p in Easy Mode
//!
//! You *probably* don't need these API docs, unless you are looking to embed
//! a VoidMerge server.
//!
//! To install the `vm` command-line utility which both runs a VoidMerge server
//! and controls / communicates with VoidMerge servers:
//!
//! `cargo install voidmerge --bin vm`
//!
//! Then get help from the commandline itself, using `vm help`.
//!
//! If you want to write a VoidMerge application,
//! see the [Typescript Client API Docs](https://voidmerge.com/ts).

pub mod error;
pub use error::{Error, ErrorExt, Result};
use std::sync::{Arc, Weak};
pub mod memindex;

/// A boxed future.
pub type BoxFut<'lt, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'lt + Send>>;

/// Current system time as f64 seconds.
/// This function will never return a duplicate number even if called
/// in a tight loop.
pub fn safe_now() -> f64 {
    static A: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);

    let mut now = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap()
        .as_secs_f64();

    let _ = A.fetch_update(
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
        |stored| {
            let mut stored = f64::from_le_bytes(stored.to_le_bytes());
            stored += 0.000001;
            if stored > now {
                now = stored;
            }
            Some(u64::from_le_bytes(now.to_le_bytes()))
        },
    );

    now
}

/// Check for safe characters to be used in contexts / paths / etc.
fn safe_str(s: &str) -> Result<()> {
    for b in s.as_bytes() {
        if (*b >= b'a' && *b <= b'z')
            || (*b >= b'A' && *b <= b'Z')
            || (*b >= b'0' && *b <= b'9')
            || *b == b'-'
            || *b == b'_'
            || *b == b'.'
            || *b == b'~'
        {
            continue;
        }
        return Err(Error::other(
            "Invalid string (can only contain [a-z], [A-Z], [0-9], '-', '_', '.', and '~')",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct RuntimeInner {
    pub obj: std::sync::OnceLock<obj::ObjWrap>,
    pub js: std::sync::OnceLock<js::DynJsExec>,
    pub msg: std::sync::OnceLock<msg::DynMsg>,
}

/// A cloneable runtime instance that can be passed to modules.
#[derive(Debug, Clone)]
pub struct Runtime(Weak<RuntimeInner>, u64);

impl std::hash::Hash for Runtime {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.1.hash(state);
    }
}

impl PartialEq for Runtime {
    fn eq(&self, oth: &Self) -> bool {
        self.1 == oth.1
    }
}

impl Eq for Runtime {}

impl Runtime {
    /// Get the obj module.
    pub fn obj(&self) -> Result<obj::ObjWrap> {
        Ok(self
            .0
            .upgrade()
            .ok_or_else(|| Error::other("closing"))?
            .obj
            .get()
            .ok_or_else(|| Error::other("closing"))?
            .clone())
    }

    /// Get the js module.
    pub fn js(&self) -> Result<js::DynJsExec> {
        Ok(self
            .0
            .upgrade()
            .ok_or_else(|| Error::other("closing"))?
            .js
            .get()
            .ok_or_else(|| Error::other("closing"))?
            .clone())
    }

    /// Get the msg module.
    pub fn msg(&self) -> Result<msg::DynMsg> {
        Ok(self
            .0
            .upgrade()
            .ok_or_else(|| Error::other("closing"))?
            .msg
            .get()
            .ok_or_else(|| Error::other("closing"))?
            .clone())
    }
}

/// VoidMerge [Runtime] manages module interdependencies.
pub struct RuntimeHandle(Arc<RuntimeInner>, u64);

impl Default for RuntimeHandle {
    fn default() -> Self {
        static UNIQ: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(1);
        Self(
            Default::default(),
            UNIQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )
    }
}

impl RuntimeHandle {
    /// Set the obj module for this runtime.
    pub fn set_obj(&self, obj: obj::ObjWrap) {
        let _ = self.0.obj.set(obj);
    }

    /// Set the js module for this runtime.
    pub fn set_js(&self, js: js::DynJsExec) {
        let _ = self.0.js.set(js);
    }

    /// Set the msg module for this runtime.
    pub fn set_msg(&self, msg: msg::DynMsg) {
        let _ = self.0.msg.set(msg);
    }

    /// Get a clonable runtime instance that can be passed to modules.
    pub fn runtime(&self) -> Runtime {
        Runtime(Arc::downgrade(&self.0), self.1)
    }
}

pub mod bytes_ext;
pub(crate) mod ctx;
pub mod http_client;
#[cfg(feature = "http-server")]
pub mod http_server;
pub mod js;
pub mod meter;
pub mod msg;
pub mod obj;
pub mod server;

use bytes_ext::BytesExt;
