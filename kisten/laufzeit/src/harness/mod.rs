//! Self-harness support modules.  The loop and governance components build on
//! these stable configuration and credential boundaries.

pub mod agent_loop;
pub mod approval;
pub mod audit;
pub mod barrier;
pub mod config;
pub mod context;
pub mod coordinator;
pub mod credentials;
pub mod evolution;
pub mod feedback;
pub mod files;
pub mod governance;
pub mod memory;
pub mod mutation;
pub mod process;
pub mod process_runtime;
mod process_tree;
pub(crate) mod private_fs;
pub mod provider;
pub mod run_store;
pub mod service;
pub(crate) mod secret_scan;
pub mod tools;
pub mod transcript;
pub mod validator;
pub mod workspace_patch;
pub mod workspace_write;

pub use feedback::{
    BuiltFeedback, FailureLoopGuard, FeedbackClass, FeedbackEngine, FeedbackInput, FeedbackLimits,
    FeedbackTruncation, LoopGuardConfigError, SecretSetId,
};
pub use mutation::{
    MutationObservation, MutationTracker, SnapshotError, SnapshotLimits, SnapshotResult,
    SourceWatchConfig, WorkspaceSnapshot, WorkspaceSnapshotter,
};
pub use validator::{
    can_finish, FinishBlocked, ProcessResult, ProcessTermination, ValidatorClassification,
    ValidatorEngine, ValidatorEvaluation, ValidatorSpec, ValidatorSpecError, ValidatorState,
};

#[cfg(test)]
#[path = "workspace_write_tests.rs"]
mod workspace_write_tests;

#[cfg(test)]
#[path = "workspace_patch_tests.rs"]
mod workspace_patch_tests;

pub use config::{
    check_permissions, merge_security, require_user_permissions, ConfigError, ConfigLoader,
    GovernanceConfig, GovernanceOverrides, LimitsConfig, LimitsOverrides, PermissionDiagnostic,
    ProjectConfig, ProjectTrustConfig, ProviderConfig, RedactedConfig, ResolvedModelProfile,
    SecretReference, SecretReferenceError, TuiConfig, UserConfig, ValidatorConfig, PROJECT_CONFIG,
    USER_CONFIG,
};
pub use credentials::{
    provider_secret, CredentialError, CredentialStore, InMemoryCredentialStore,
    KeyringCredentialStore, ProviderSecret, KEYRING_SERVICE,
};
