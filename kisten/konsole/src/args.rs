//! Command-line surface for Orchester.
//!
//! The v0.1 grammar is intentionally tiny: `run`, `list`, `doctor`, `sessions`,
//! plus a default "run" mode. Multi-agent flags (`--agents`, `--parallel`, `--auto`)
//! are declared but stubbed: they lock the UX now and print "not yet
//! implemented" so scripts written against them fail loudly rather than
//! silently doing the wrong thing.

use clap::{Args, Parser, Subcommand};

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
    /// Run one adapter with a prompt.
    Run(RunArgs),

    /// List discovered adapters and their capabilities.
    List,

    /// Check local adapter availability.
    Doctor(DoctorArgs),

    /// List locally recorded session metadata.
    Sessions,

    /// Inspect and manage agent plugin packages.
    Plugin(PluginArgs),

    /// Show the resolved self-agent configuration with secrets redacted.
    Config,

    /// Store a provider API key in the OS keyring.
    Login(CredentialArgs),

    /// Forget a provider API key held in the OS keyring.
    Logout(CredentialArgs),
}

#[derive(Debug, Args)]
pub struct CredentialArgs {
    /// Provider name. Defaults to the one the effective config marks active.
    pub provider: Option<String>,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// The prompt. Use `-` to read the prompt from stdin.
    pub prompt: Option<String>,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Exit non-zero if any adapter command is missing.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

#[derive(Debug, Subcommand)]
pub enum PluginCommand {
    /// List validated plugin packages discovered at startup.
    List,
    /// Show one validated plugin package.
    Status(PluginStatusArgs),
    /// Install a validated agent plugin into Orchester's managed store.
    Install(PluginInstallArgs),
    /// Remove a receipted plugin from Orchester's managed store.
    Remove(PluginRemoveArgs),
}

#[derive(Debug, Args)]
pub struct PluginStatusArgs {
    /// Plugin name without the `@orchester/` scope.
    pub name: String,
}

#[derive(Debug, Args)]
pub struct PluginInstallArgs {
    /// Plugin name without the `@orchester/` scope.
    pub name: String,
}

#[derive(Debug, Args)]
pub struct PluginRemoveArgs {
    /// Plugin name without the `@orchester/` scope.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn provider_of(argv: &[&str]) -> Option<String> {
        match Cli::try_parse_from(argv).expect("parse").command {
            Some(Command::Login(args) | Command::Logout(args)) => args.provider,
            other => panic!("expected a credential command, got {other:?}"),
        }
    }

    #[test]
    fn credential_commands_default_to_the_active_provider() {
        assert_eq!(provider_of(&["orchester", "login"]), None);
        assert_eq!(provider_of(&["orchester", "logout"]), None);
    }

    #[test]
    fn a_named_provider_is_carried_through() {
        assert_eq!(
            provider_of(&["orchester", "login", "OpenAI"]),
            Some("OpenAI".into())
        );
    }

    #[test]
    fn a_second_positional_is_rejected_rather_than_silently_dropped() {
        // `login OpenAI sk-live-…` is a plausible typo, and accepting it would
        // leave the key in the shell history while storing nothing.
        assert!(Cli::try_parse_from(["orchester", "login", "OpenAI", "extra"]).is_err());
    }

    #[test]
    fn the_command_surface_stays_valid() {
        Cli::command().debug_assert();
    }
}
