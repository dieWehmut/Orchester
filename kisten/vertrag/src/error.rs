use thiserror::Error;

/// Errors an adapter can surface while spawning or streaming from an agent.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// The agent binary could not be spawned (missing CLI, bad cwd, …).
    #[error("failed to spawn agent `{command}`: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },

    /// I/O failure while reading the agent's stdout stream.
    #[error("i/o error reading agent output: {0}")]
    Io(#[from] std::io::Error),

    /// A manifest referenced a template placeholder or path we can't satisfy.
    #[error("manifest error: {0}")]
    Manifest(String),

    /// Failed to parse a manifest TOML file.
    #[error("invalid manifest: {0}")]
    ManifestParse(#[from] toml::de::Error),

    /// The agent produced a line that could not be handled.
    #[error("failed to parse agent output line: {0}")]
    Parse(String),
}
