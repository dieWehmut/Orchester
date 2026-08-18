//! Where Orchester keeps its files.
//!
//! The runtime resolves the home directory, and until now each caller joined its
//! own file names onto it: `main.rs` knew that the run database is
//! `state/runs.db` and the session log is `sessions.jsonl`, and nothing else
//! did. That is fine for one frontend and wrong for three — a server and a
//! desktop shell that guess differently would each open their own database and
//! silently disagree about what sessions exist.
//!
//! So the layout lives here, once, as data.

use std::io;
use std::path::{Path, PathBuf};

use orchester_laufzeit::harness::{orchester_home, ORCHESTER_DIR};

/// The directory Orchester keeps state in, plus the workspace a run acts on.
///
/// Two roots, because they answer different questions: the home is where
/// Orchester's own files live (one per user), and the workspace is the project a
/// run reads and writes (one per window, tab or terminal). A server serving two
/// projects shares the first and not the second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchesterPaths {
    home: PathBuf,
    workspace: PathBuf,
}

impl OrchesterPaths {
    /// Resolve both roots from the environment: the home from `ORCHESTER_HOME`
    /// or the user's home directory, the workspace from the current directory.
    ///
    /// A home that cannot be resolved stays the relative `.orchester` rather
    /// than becoming an error. That looks careless and is deliberate: the plugin
    /// layer already rejects a relative home with a message that does not echo
    /// the offending path, and failing here instead would replace that message
    /// with a worse one and take `orchester --help` down with it.
    pub fn discover() -> io::Result<Self> {
        Ok(Self::new(
            orchester_home().unwrap_or_else(|| PathBuf::from(ORCHESTER_DIR)),
            std::env::current_dir()?,
        ))
    }

    /// Both roots given explicitly, for tests and for a frontend that lets the
    /// user pick a project directory.
    pub fn new(home: impl Into<PathBuf>, workspace: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            workspace: workspace.into(),
        }
    }

    /// The same home, pointed at another project.
    pub fn with_workspace(&self, workspace: impl Into<PathBuf>) -> Self {
        Self::new(self.home.clone(), workspace)
    }

    /// Where Orchester's own files live.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The project a run reads and writes.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Everything a run produces that is not meant to be edited by hand.
    ///
    /// Separate from the home so the configuration file a user does edit is not
    /// buried among databases and logs.
    pub fn state_root(&self) -> PathBuf {
        self.home.join("state")
    }

    /// The durable store of governed runs: turns, actions, observations.
    pub fn run_database(&self) -> PathBuf {
        self.state_root().join("runs.db")
    }

    /// The append-only record of every governed action that executed.
    pub fn audit_log(&self) -> PathBuf {
        self.state_root().join("audit.jsonl")
    }

    /// The adapter path's session history, one JSON object per finished run.
    ///
    /// Not under [`Self::state_root`]: it predates it, and moving it would make
    /// an upgrade look like it had erased the user's history.
    pub fn session_log(&self) -> PathBuf {
        self.home.join("sessions.jsonl")
    }

    /// Where a project keeps its own adapter manifests, overriding the built-ins.
    pub fn manifest_dir(&self) -> PathBuf {
        self.workspace.join(MANIFEST_DIR)
    }
}

/// The per-project manifest directory name.
const MANIFEST_DIR: &str = "manifeste";

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> OrchesterPaths {
        OrchesterPaths::new("/home/u/.orchester", "/work/project")
    }

    #[test]
    fn state_files_share_one_root() {
        let paths = paths();
        let state = paths.state_root();
        assert!(paths.run_database().starts_with(&state));
        assert!(paths.audit_log().starts_with(&state));
    }

    #[test]
    fn the_session_log_stays_beside_the_config() {
        // Moving it under state/ would read as lost history on upgrade.
        let paths = paths();
        assert_eq!(paths.session_log().parent(), Some(paths.home()));
    }

    #[test]
    fn manifests_are_per_project_not_per_user() {
        let paths = paths();
        assert!(paths.manifest_dir().starts_with(paths.workspace()));
    }

    #[test]
    fn another_project_keeps_the_same_home() {
        let paths = paths();
        let other = paths.with_workspace("/work/other");
        assert_eq!(other.home(), paths.home());
        assert_eq!(other.run_database(), paths.run_database());
        assert_ne!(other.manifest_dir(), paths.manifest_dir());
    }
}
