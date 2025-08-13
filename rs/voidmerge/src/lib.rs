#![deny(missing_docs)]
//! voidmerge

use bytes::Bytes;
use std::io::Result;
use std::sync::{Arc, Mutex};

pub mod config;
pub mod context;
pub mod http_client;
#[cfg(feature = "http-server")]
pub mod http_server;
pub mod modules;
pub mod runtime;
pub mod server;
pub mod types;

#[cfg(test)]
mod test;
