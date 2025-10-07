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
    if s.is_empty() {
        return Err(Error::other("string cannot be empty"));
    }
    for b in s.as_bytes() {
        if (*b >= b'a' && *b <= b'z')
            || (*b >= b'A' && *b <= b'Z')
            || (*b >= b'0' && *b <= b'9')
            || *b == b'-'
            || *b == b'_'
            || *b == b'~'
        {
            continue;
        }
        return Err(Error::other(
            "Invalid string (can only contain [a-z], [A-Z], [0-9], '-', '_', and '~')",
        ));
    }
    Ok(())
}

pub mod bytes_ext;
pub(crate) mod ctx;
pub mod http_client;
#[cfg(feature = "http-server")]
pub mod http_server;
pub mod js;
pub mod obj;
pub mod server;

use bytes_ext::BytesExt;

/*
use bytes::Bytes;
use std::io::Result;
use std::sync::{Arc, Mutex};

pub mod config;
pub mod context;
pub mod crypto;
pub mod data;
pub mod http_client;
#[cfg(feature = "http-server")]
pub mod http_server;
pub mod modules;
pub mod runtime;
pub mod server;
pub mod types;

#[cfg(test)]
mod test;
*/
