//! Self-harness support modules.  The loop and governance components build on
//! these stable configuration and credential boundaries.

pub mod approval;
pub mod audit;
pub mod barrier;
pub mod config;
pub mod context;
pub mod credentials;
pub mod feedback;
pub mod governance;
pub mod memory;
pub mod mutation;
pub mod run_store;
pub mod validator;

pub use feedback::{
    BuiltFeedback, FailureLoopGuard, FeedbackClass, FeedbackEngine, FeedbackInput, FeedbackLimits,
    FeedbackTruncation, LoopGuardConfigError,
};
pub use mutation::{
    MutationObservation, MutationTracker, SnapshotError, SnapshotLimits, SnapshotResult,
    SourceWatchConfig, WorkspaceSnapshot, WorkspaceSnapshotter,
};
pub use validator::{
    can_finish, FinishBlocked, ProcessResult, ProcessTermination, ValidatorClassification,
    ValidatorEngine, ValidatorEvaluation, ValidatorSpec, ValidatorSpecError, ValidatorState,
};

pub use config::{
    check_permissions, merge_security, require_user_permissions, ConfigError, ConfigLoader,
    GovernanceConfig, GovernanceOverrides, LimitsConfig, LimitsOverrides, PermissionDiagnostic,
    ProjectConfig, ProjectTrustConfig, ProviderConfig, RedactedConfig, SecretReference,
    SecretReferenceError, TuiConfig, UserConfig, ValidatorConfig, PROJECT_CONFIG, USER_CONFIG,
};
pub use credentials::{
    provider_secret, CredentialError, CredentialStore, InMemoryCredentialStore,
    KeyringCredentialStore, ProviderSecret, KEYRING_SERVICE,
};
