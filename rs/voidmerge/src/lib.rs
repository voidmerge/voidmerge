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
fn sys_now() -> f64 {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .expect("system time error")
        .as_secs_f64()
}

/// Check for safe characters to be used in contexts / paths / etc.
fn safe_str(s: &str) -> Result<()> {
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
