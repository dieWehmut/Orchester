//! # Vertrag — the adapter contract
//!
//! This crate defines *how Orchester talks to any agent*:
//! - [`AgentAdapter`] — the async trait every adapter implements.
//! - [`AdapterManifest`] — a declarative TOML description of a subprocess agent.
//! - [`ManifestAdapter`] — a generic engine that turns any manifest into a working
//!   adapter with **no new Rust code** (the "data by default" half of the hybrid model).
//! - [`extract`] — the dotted/indexed JSON path extractor the manifest engine uses.
//!
//! The three built-in vendors (claude/codex/opencode) are just manifests fed to
//! `ManifestAdapter`; only genuinely irregular vendors need a bespoke Rust adapter.

mod adapter;
mod error;
pub mod extract;
mod manifest;
mod manifest_adapter;
pub mod process;

pub use adapter::{AdapterAvailability, AgentAdapter, AvailabilityStatus, EventStream};
pub use error::AdapterError;
pub use manifest::{AdapterManifest, EventMapping, ParseSpec};
pub use manifest_adapter::ManifestAdapter;
