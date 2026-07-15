use std::fmt;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::EvolutionError;
use super::identity::{CandidateId, Encoder, EvolutionDigest, RevisionId};
use super::validation::{
    MAX_FORMAT_BYTES, MAX_IDENTIFIER_BYTES, expiry_is_later, validate_text, validate_timestamp,
};

const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const CANDIDATE_DOMAIN: &[u8] = b"orchester-candidate-id-v1";

#[derive(Clone, PartialEq, Eq)]
pub struct EvolutionScope {
    project_id: String,
    owner_actor_id: String,
}

impl EvolutionScope {
    pub fn try_new(
        project_id: impl AsRef<str>,
        owner_actor_id: impl AsRef<str>,
    ) -> Result<Self, EvolutionError> {
        Ok(Self {
            project_id: validate_text(project_id.as_ref(), MAX_IDENTIFIER_BYTES)?,
            owner_actor_id: validate_text(owner_actor_id.as_ref(), MAX_IDENTIFIER_BYTES)?,
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn owner_actor_id(&self) -> &str {
        &self.owner_actor_id
    }
}

impl fmt::Debug for EvolutionScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvolutionScope")
            .field("project_id_bytes", &self.project_id.len())
            .field("owner_actor_id_bytes", &self.owner_actor_id.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    digest: EvolutionDigest,
    byte_len: usize,
    format: String,
}

impl ArtifactRef {
    pub fn try_new(
        digest: EvolutionDigest,
        byte_len: usize,
        format: impl AsRef<str>,
    ) -> Result<Self, EvolutionError> {
        if byte_len == 0 || byte_len > MAX_ARTIFACT_BYTES {
            return Err(EvolutionError::InvalidInput);
        }
        Ok(Self {
            digest,
            byte_len,
            format: validate_text(format.as_ref(), MAX_FORMAT_BYTES)?,
        })
    }

    pub fn digest(&self) -> &EvolutionDigest {
        &self.digest
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn format(&self) -> &str {
        &self.format
    }
}

impl fmt::Debug for ArtifactRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArtifactRef")
            .field("digest", &self.digest)
            .field("byte_len", &self.byte_len)
            .field("format_bytes", &self.format.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Strategy,
    Prompt,
    ToolPolicy,
}

impl CandidateKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Strategy => "strategy",
            Self::Prompt => "prompt",
            Self::ToolPolicy => "tool_policy",
        }
    }
}

impl Serialize for CandidateKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CandidateKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)
            .map_err(|_| D::Error::custom(EvolutionError::InvalidInput))?;
        match value.as_str() {
            "strategy" => Ok(Self::Strategy),
            "prompt" => Ok(Self::Prompt),
            "tool_policy" => Ok(Self::ToolPolicy),
            _ => Err(D::Error::custom(EvolutionError::InvalidInput)),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CandidateManifestV1 {
    scope: EvolutionScope,
    kind: CandidateKind,
    parent_revision: Option<RevisionId>,
    artifact: ArtifactRef,
    created_at: String,
    expires_at: String,
    candidate_id: CandidateId,
}

impl CandidateManifestV1 {
    pub fn try_new(
        scope: EvolutionScope,
        kind: CandidateKind,
        parent_revision: Option<RevisionId>,
        artifact: ArtifactRef,
        created_at: impl AsRef<str>,
        expires_at: impl AsRef<str>,
    ) -> Result<Self, EvolutionError> {
        let created_at = validate_timestamp(created_at.as_ref())?;
        let expires_at = validate_timestamp(expires_at.as_ref())?;
        if !expiry_is_later(&created_at, &expires_at)? {
            return Err(EvolutionError::InvalidExpiry);
        }
        let mut encoder = Encoder::new(CANDIDATE_DOMAIN);
        encoder.field(scope.project_id());
        encoder.field(scope.owner_actor_id());
        encoder.field(kind.as_str());
        encoder.optional(parent_revision.as_ref().map(RevisionId::as_str));
        encoder.field(artifact.digest().as_str());
        encoder.number(artifact.byte_len());
        encoder.field(artifact.format());
        encoder.field(&created_at);
        encoder.field(&expires_at);
        let candidate_id = CandidateId::from_preimage(&encoder.finish());
        Ok(Self {
            scope,
            kind,
            parent_revision,
            artifact,
            created_at,
            expires_at,
            candidate_id,
        })
    }

    pub fn candidate_id(&self) -> &CandidateId {
        &self.candidate_id
    }

    pub fn scope(&self) -> &EvolutionScope {
        &self.scope
    }

    pub fn kind(&self) -> CandidateKind {
        self.kind
    }

    pub fn parent_revision(&self) -> Option<&RevisionId> {
        self.parent_revision.as_ref()
    }

    pub fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn expires_at(&self) -> &str {
        &self.expires_at
    }
}

impl fmt::Debug for CandidateManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateManifestV1")
            .field("scope", &self.scope)
            .field("kind", &self.kind)
            .field("parent_revision", &self.parent_revision)
            .field("artifact", &self.artifact)
            .field("created_at_bytes", &self.created_at.len())
            .field("expires_at_bytes", &self.expires_at.len())
            .field("candidate_id", &self.candidate_id)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateManifestWire {
    schema_version: u16,
    project_id: String,
    owner_actor_id: String,
    kind: CandidateKind,
    parent_revision: Option<RevisionId>,
    artifact_digest: EvolutionDigest,
    artifact_bytes: usize,
    artifact_format: String,
    created_at: String,
    expires_at: String,
    candidate_id: EvolutionDigest,
}

impl Serialize for CandidateManifestV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        CandidateManifestWire {
            schema_version: 1,
            project_id: self.scope.project_id.clone(),
            owner_actor_id: self.scope.owner_actor_id.clone(),
            kind: self.kind,
            parent_revision: self.parent_revision.clone(),
            artifact_digest: self.artifact.digest.clone(),
            artifact_bytes: self.artifact.byte_len,
            artifact_format: self.artifact.format.clone(),
            created_at: self.created_at.clone(),
            expires_at: self.expires_at.clone(),
            candidate_id: self.candidate_id.digest().clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CandidateManifestV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CandidateManifestWire::deserialize(deserializer)
            .map_err(|_| D::Error::custom(EvolutionError::InvalidInput))?;
        if wire.schema_version != 1 {
            return Err(D::Error::custom(EvolutionError::UnsupportedSchema));
        }
        let scope = EvolutionScope::try_new(wire.project_id, wire.owner_actor_id)
            .map_err(D::Error::custom)?;
        let artifact = ArtifactRef::try_new(
            wire.artifact_digest,
            wire.artifact_bytes,
            wire.artifact_format,
        )
        .map_err(D::Error::custom)?;
        let manifest = Self::try_new(
            scope,
            wire.kind,
            wire.parent_revision,
            artifact,
            wire.created_at,
            wire.expires_at,
        )
        .map_err(D::Error::custom)?;
        if manifest.candidate_id.digest() != &wire.candidate_id {
            return Err(D::Error::custom(EvolutionError::Corrupt));
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateState {
    Proposed,
    Evaluating,
    Evaluated,
    Quarantined,
    Expired,
}

impl CandidateState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Evaluating => "evaluating",
            Self::Evaluated => "evaluated",
            Self::Quarantined => "quarantined",
            Self::Expired => "expired",
        }
    }
}

impl Serialize for CandidateState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CandidateState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)
            .map_err(|_| D::Error::custom(EvolutionError::InvalidInput))?;
        match value.as_str() {
            "proposed" => Ok(Self::Proposed),
            "evaluating" => Ok(Self::Evaluating),
            "evaluated" => Ok(Self::Evaluated),
            "quarantined" => Ok(Self::Quarantined),
            "expired" => Ok(Self::Expired),
            _ => Err(D::Error::custom(EvolutionError::InvalidInput)),
        }
    }
}

pub fn can_transition(from: CandidateState, to: CandidateState) -> bool {
    matches!(
        (from, to),
        (CandidateState::Proposed, CandidateState::Evaluating)
            | (CandidateState::Proposed, CandidateState::Quarantined)
            | (CandidateState::Proposed, CandidateState::Expired)
            | (CandidateState::Evaluating, CandidateState::Evaluated)
            | (CandidateState::Evaluating, CandidateState::Quarantined)
            | (CandidateState::Evaluating, CandidateState::Expired)
            | (CandidateState::Evaluated, CandidateState::Quarantined)
            | (CandidateState::Evaluated, CandidateState::Expired)
    )
}
