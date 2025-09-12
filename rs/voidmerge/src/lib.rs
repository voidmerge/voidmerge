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

use bytes::Bytes;
use std::io::Result;
use std::sync::{Arc, Mutex};

pub mod config;
pub mod context;
pub mod crypto;
pub mod http_client;
#[cfg(feature = "http-server")]
pub mod http_server;
pub mod modules;
pub mod runtime;
pub mod server;
pub mod types;

#[cfg(test)]
mod test;
