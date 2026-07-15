use std::fmt;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use super::EvolutionError;

const DIGEST_HEX_BYTES: usize = 64;
const REVISION_DOMAIN: &[u8] = b"orchester-revision-id-v1";

#[derive(Clone, PartialEq, Eq)]
pub struct EvolutionDigest(String);

impl EvolutionDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn from_preimage(bytes: &[u8]) -> Self {
        Self(hex(&Sha256::digest(bytes)))
    }
}

impl TryFrom<&str> for EvolutionDigest {
    type Error = EvolutionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != DIGEST_HEX_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(EvolutionError::InvalidDigest);
        }
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for EvolutionDigest {
    type Error = EvolutionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl fmt::Debug for EvolutionDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EvolutionDigest(<redacted>)")
    }
}

impl fmt::Display for EvolutionDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for EvolutionDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for EvolutionDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

/// A candidate identity that can only be obtained from a validated manifest.
///
/// ```compile_fail
/// use orchester_laufzeit::harness::evolution::CandidateId;
///
/// let forged: CandidateId = serde_json::from_str(
///     "\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
/// )?;
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct CandidateId(EvolutionDigest);

impl CandidateId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(super) fn from_preimage(bytes: &[u8]) -> Self {
        Self(EvolutionDigest::from_preimage(bytes))
    }

    pub(super) fn digest(&self) -> &EvolutionDigest {
        &self.0
    }
}

impl fmt::Debug for CandidateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CandidateId(<redacted>)")
    }
}

impl Serialize for CandidateId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// An evaluation identity that can only be obtained from a validated key.
///
/// ```compile_fail
/// use orchester_laufzeit::harness::evolution::EvaluationId;
///
/// let forged: EvaluationId = serde_json::from_str(
///     "\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
/// )?;
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct EvaluationId(EvolutionDigest);

impl EvaluationId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(super) fn from_preimage(bytes: &[u8]) -> Self {
        Self(EvolutionDigest::from_preimage(bytes))
    }

    pub(super) fn digest(&self) -> &EvolutionDigest {
        &self.0
    }
}

impl fmt::Debug for EvaluationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EvaluationId(<redacted>)")
    }
}

impl Serialize for EvaluationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RevisionId(EvolutionDigest);

impl RevisionId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn derive(
        candidate_id: &CandidateId,
        evaluation_id: &EvaluationId,
        parent_revision: Option<&RevisionId>,
    ) -> Self {
        let mut encoder = Encoder::new(REVISION_DOMAIN);
        encoder.field(candidate_id.as_str());
        encoder.field(evaluation_id.as_str());
        encoder.optional(parent_revision.map(Self::as_str));
        Self(EvolutionDigest::from_preimage(&encoder.finish()))
    }
}

impl TryFrom<&str> for RevisionId {
    type Error = EvolutionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(EvolutionDigest::try_from(value)?))
    }
}

impl TryFrom<String> for RevisionId {
    type Error = EvolutionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl fmt::Debug for RevisionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RevisionId(<redacted>)")
    }
}

impl Serialize for RevisionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RevisionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

pub(super) struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut encoder = Self { bytes: Vec::new() };
        encoder.field_bytes(domain);
        encoder
    }

    pub(super) fn field(&mut self, value: &str) {
        self.field_bytes(value.as_bytes());
    }

    pub(super) fn field_bytes(&mut self, value: &[u8]) {
        self.bytes
            .extend_from_slice(&(value.len() as u64).to_be_bytes());
        self.bytes.extend_from_slice(value);
    }

    pub(super) fn number(&mut self, value: usize) {
        self.bytes.extend_from_slice(&(value as u64).to_be_bytes());
    }

    pub(super) fn optional(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.bytes.push(1);
                self.field(value);
            }
            None => self.bytes.push(0),
        }
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
