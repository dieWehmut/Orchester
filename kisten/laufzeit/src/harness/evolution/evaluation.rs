use std::fmt;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::EvolutionError;
use super::candidate::CandidateManifestV1;
use super::identity::{CandidateId, Encoder, EvaluationId, EvolutionDigest};
use super::snapshots::{EvaluationSnapshotInput, EvaluationSnapshots};

const EVALUATION_DOMAIN: &[u8] = b"orchester-evaluation-id-v1";

#[derive(Clone, PartialEq, Eq)]
pub struct EvaluationKey {
    candidate: CandidateManifestV1,
    snapshots: EvaluationSnapshots,
    evaluation_id: EvaluationId,
}

impl EvaluationKey {
    pub fn try_new(
        candidate: CandidateManifestV1,
        snapshots: EvaluationSnapshotInput,
    ) -> Result<Self, EvolutionError> {
        let snapshots = snapshots.require_complete()?;
        let mut encoder = Encoder::new(EVALUATION_DOMAIN);
        encoder.field(candidate.candidate_id().as_str());
        encoder.field(snapshots.corpus().as_str());
        encoder.field(snapshots.evaluator().as_str());
        encoder.field(snapshots.config().as_str());
        encoder.field(snapshots.policy().as_str());
        encoder.field(snapshots.catalog().as_str());
        encoder.field(snapshots.environment().as_str());
        let evaluation_id = EvaluationId::from_preimage(&encoder.finish());
        Ok(Self {
            candidate,
            snapshots,
            evaluation_id,
        })
    }

    pub fn candidate_id(&self) -> &CandidateId {
        self.candidate.candidate_id()
    }

    pub fn candidate(&self) -> &CandidateManifestV1 {
        &self.candidate
    }

    pub fn evaluation_id(&self) -> &EvaluationId {
        &self.evaluation_id
    }

    pub fn snapshots(&self) -> &EvaluationSnapshots {
        &self.snapshots
    }
}

impl fmt::Debug for EvaluationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvaluationKey")
            .field("candidate", &self.candidate)
            .field("snapshots", &self.snapshots)
            .field("evaluation_id", &self.evaluation_id)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationKeyWire {
    schema_version: u16,
    candidate: CandidateManifestV1,
    corpus_hash: EvolutionDigest,
    evaluator_hash: EvolutionDigest,
    config_hash: EvolutionDigest,
    policy_hash: EvolutionDigest,
    catalog_hash: EvolutionDigest,
    environment_hash: EvolutionDigest,
    evaluation_id: EvolutionDigest,
}

impl Serialize for EvaluationKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        EvaluationKeyWire {
            schema_version: 1,
            candidate: self.candidate.clone(),
            corpus_hash: self.snapshots.corpus().clone(),
            evaluator_hash: self.snapshots.evaluator().clone(),
            config_hash: self.snapshots.config().clone(),
            policy_hash: self.snapshots.policy().clone(),
            catalog_hash: self.snapshots.catalog().clone(),
            environment_hash: self.snapshots.environment().clone(),
            evaluation_id: self.evaluation_id.digest().clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EvaluationKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EvaluationKeyWire::deserialize(deserializer)
            .map_err(|_| D::Error::custom(EvolutionError::InvalidInput))?;
        if wire.schema_version != 1 {
            return Err(D::Error::custom(EvolutionError::UnsupportedSchema));
        }
        let key = Self::try_new(
            wire.candidate,
            EvaluationSnapshotInput::new()
                .with_corpus(wire.corpus_hash)
                .with_evaluator(wire.evaluator_hash)
                .with_config(wire.config_hash)
                .with_policy(wire.policy_hash)
                .with_catalog(wire.catalog_hash)
                .with_environment(wire.environment_hash),
        )
        .map_err(D::Error::custom)?;
        if key.evaluation_id.digest() != &wire.evaluation_id {
            return Err(D::Error::custom(EvolutionError::Corrupt));
        }
        Ok(key)
    }
}
