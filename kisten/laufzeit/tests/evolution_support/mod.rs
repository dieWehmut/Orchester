#![allow(dead_code)]

pub use orchester_laufzeit::harness::evolution::{
    ArtifactRef, CandidateKind, CandidateManifestV1, EvaluationKey, EvaluationSnapshotInput,
    EvolutionDigest, EvolutionScope, RevisionId,
};

pub const CREATED_AT: &str = "2026-07-15T10:00:00Z";
pub const EXPIRES_AT: &str = "2026-07-16T10:00:00Z";

pub fn digest(fill: char) -> EvolutionDigest {
    EvolutionDigest::try_from(fill.to_string().repeat(64)).unwrap()
}

pub fn scope() -> EvolutionScope {
    EvolutionScope::try_new("project-a", "owner-a").unwrap()
}

pub fn artifact(fill: char, byte_len: usize, format: &str) -> ArtifactRef {
    ArtifactRef::try_new(digest(fill), byte_len, format).unwrap()
}

pub fn parent_revision() -> RevisionId {
    RevisionId::try_from("d".repeat(64)).unwrap()
}

pub fn manifest(
    scope: EvolutionScope,
    kind: CandidateKind,
    parent_revision: Option<RevisionId>,
    artifact: ArtifactRef,
    created_at: &str,
    expires_at: &str,
) -> CandidateManifestV1 {
    CandidateManifestV1::try_new(
        scope,
        kind,
        parent_revision,
        artifact,
        created_at,
        expires_at,
    )
    .unwrap()
}

pub fn base_manifest() -> CandidateManifestV1 {
    manifest(
        scope(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "json"),
        CREATED_AT,
        EXPIRES_AT,
    )
}

#[derive(Clone)]
pub struct SnapshotFixture {
    pub corpus: Option<EvolutionDigest>,
    pub evaluator: Option<EvolutionDigest>,
    pub config: Option<EvolutionDigest>,
    pub policy: Option<EvolutionDigest>,
    pub catalog: Option<EvolutionDigest>,
    pub environment: Option<EvolutionDigest>,
}

impl SnapshotFixture {
    pub fn complete() -> Self {
        Self {
            corpus: Some(digest('1')),
            evaluator: Some(digest('2')),
            config: Some(digest('3')),
            policy: Some(digest('4')),
            catalog: Some(digest('5')),
            environment: Some(digest('6')),
        }
    }

    pub fn input(self) -> EvaluationSnapshotInput {
        let mut input = EvaluationSnapshotInput::new();
        if let Some(value) = self.corpus {
            input = input.with_corpus(value);
        }
        if let Some(value) = self.evaluator {
            input = input.with_evaluator(value);
        }
        if let Some(value) = self.config {
            input = input.with_config(value);
        }
        if let Some(value) = self.policy {
            input = input.with_policy(value);
        }
        if let Some(value) = self.catalog {
            input = input.with_catalog(value);
        }
        if let Some(value) = self.environment {
            input = input.with_environment(value);
        }
        input
    }
}

pub fn snapshots() -> EvaluationSnapshotInput {
    SnapshotFixture::complete().input()
}

pub fn evaluation(
    candidate: &CandidateManifestV1,
    snapshots: EvaluationSnapshotInput,
) -> EvaluationKey {
    EvaluationKey::try_new(candidate.clone(), snapshots).unwrap()
}

pub fn assert_lower_hex(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}
