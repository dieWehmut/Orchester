#![forbid(unsafe_code)]

//! Loopback HTTP and WebSocket service for Orchester frontends.

mod config;

pub use config::{ServerConfig, StaticAssets};
