//! Immutable identities used by the controlled harness-evolution data plane.
//!
//! This module deliberately contains no evaluator, artifact writer, publish
//! path, or active-revision mutation. Those capabilities are added behind
//! separate reviewed boundaries.

mod candidate;
mod error;
mod evaluation;
mod identity;
mod snapshots;
mod validation;

pub use candidate::{
    ArtifactRef, CandidateKind, CandidateManifestV1, CandidateState, EvolutionScope, can_transition,
};
pub use error::EvolutionError;
pub use evaluation::EvaluationKey;
pub use identity::{CandidateId, EvaluationId, EvolutionDigest, RevisionId};
pub use snapshots::{EvaluationSnapshotInput, EvaluationSnapshots};
