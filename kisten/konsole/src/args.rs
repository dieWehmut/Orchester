//! Command-line surface for Orchester.
//!
//! The v0.1 grammar is intentionally tiny — one subcommand (`list`) plus a
//! default "run" mode. Multi-agent flags (`--agents`, `--parallel`, `--auto`)
//! are declared but stubbed: they lock the UX now and print "not yet
//! implemented" so scripts written against them fail loudly rather than
//! silently doing the wrong thing.

use clap::{Parser, Subcommand};

/// Orchester — a conductor for heterogeneous coding agents.
#[derive(Debug, Parser)]
#[command(name = "orchester", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Agent to run (e.g. `mock`, `claude`, `codex`, `opencode`).
    #[arg(long, short = 'a', global = true)]
    pub agent: Option<String>,

    /// Resume a prior session by id.
    #[arg(long, global = true)]
    pub resume: Option<String>,

    /// Override the model (vendor-specific string).
    #[arg(long, short = 'm', global = true)]
    pub model: Option<String>,

    /// Emit Orchester's own Event JSONL instead of rendered output.
    #[arg(long, global = true)]
    pub json: bool,

    // --- reserved for later roadmap stages (parsed, not yet wired) ---
    /// [v0.5] Run several agents at once (comma-separated).
    #[arg(long, global = true)]
    pub agents: Option<String>,
    /// [v0.5] Run the selected agents in parallel.
    #[arg(long, global = true)]
    pub parallel: bool,
    /// [v1.0] Let the planner choose the agent automatically.
    #[arg(long, global = true)]
    pub auto: bool,

    /// The prompt. Use `-` to read the prompt from stdin.
    pub prompt: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List discovered adapters and their capabilities.
    List,
}
