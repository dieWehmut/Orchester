use orchester_protokoll::normalize_action_summary;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::AuditError;

pub(super) const CURRENT_SCHEMA_VERSION: u16 = 2;
const LEGACY_SCHEMA_VERSION: u16 = 1;
const MAX_ACTION_SUMMARY_BYTES: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct AuditEntryV1 {
    pub(super) schema_version: u16,
    pub(super) sequence: u64,
    pub(super) occurred_at: String,
    pub(super) event_id: String,
    pub(super) actor: String,
    pub(super) run_id: String,
    pub(super) action_id: Option<String>,
    pub(super) approval_id: Option<String>,
    pub(super) policy_rule: String,
    pub(super) decision: String,
    pub(super) result_summary: String,
    pub(super) prev_hash: String,
    pub(super) entry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct AuditEntryV2 {
    pub(super) schema_version: u16,
    pub(super) sequence: u64,
    pub(super) occurred_at: String,
    pub(super) event_id: String,
    pub(super) actor: String,
    pub(super) run_id: String,
    pub(super) action_id: Option<String>,
    pub(super) action_summary: Option<String>,
    pub(super) action_hash: Option<String>,
    pub(super) approval_id: Option<String>,
    pub(super) policy_rule: String,
    pub(super) decision: String,
    pub(super) result_summary: String,
    pub(super) prev_hash: String,
    pub(super) entry_hash: String,
}

pub(super) enum AuditRecord {
    V1(AuditEntryV1),
    V2(AuditEntryV2),
}

#[derive(Deserialize)]
struct VersionProbe {
    schema_version: u16,
}

impl AuditRecord {
    pub(super) fn parse(line: &str) -> Result<Self, AuditError> {
        // Probe the version from the original JSON, then deserialize the
        // original bytes again so duplicate fields cannot be hidden by a
        // serde_json::Value map.
        let version = serde_json::from_str::<VersionProbe>(line)
            .map_err(|_| AuditError::Corrupt)?
            .schema_version;
        match version {
            LEGACY_SCHEMA_VERSION => {
                let entry: AuditEntryV1 =
                    serde_json::from_str(line).map_err(|_| AuditError::Corrupt)?;
                Ok(Self::V1(entry))
            }
            CURRENT_SCHEMA_VERSION => {
                let entry: AuditEntryV2 =
                    serde_json::from_str(line).map_err(|_| AuditError::Corrupt)?;
                Ok(Self::V2(entry))
            }
            _ => Err(AuditError::Corrupt),
        }
    }

    pub(super) fn sequence(&self) -> u64 {
        match self {
            Self::V1(entry) => entry.sequence,
            Self::V2(entry) => entry.sequence,
        }
    }

    pub(super) fn event_id(&self) -> &str {
        match self {
            Self::V1(entry) => &entry.event_id,
            Self::V2(entry) => &entry.event_id,
        }
    }

    pub(super) fn previous_hash(&self) -> &str {
        match self {
            Self::V1(entry) => &entry.prev_hash,
            Self::V2(entry) => &entry.prev_hash,
        }
    }

    pub(super) fn entry_hash(&self) -> &str {
        match self {
            Self::V1(entry) => &entry.entry_hash,
            Self::V2(entry) => &entry.entry_hash,
        }
    }

    pub(super) fn is_current(&self) -> bool {
        matches!(self, Self::V2(_))
    }

    pub(super) fn hash(&self) -> Result<[u8; 32], AuditError> {
        match self {
            Self::V1(entry) => {
                hash_without_signature(entry, |unsigned| unsigned.entry_hash.clear())
            }
            Self::V2(entry) => {
                hash_without_signature(entry, |unsigned| unsigned.entry_hash.clear())
            }
        }
    }

    pub(super) fn validate_semantics(&self) -> Result<(), AuditError> {
        if let Self::V2(entry) = self {
            let required_ids = [
                entry.event_id.as_str(),
                entry.actor.as_str(),
                entry.run_id.as_str(),
                entry.policy_rule.as_str(),
                entry.decision.as_str(),
            ];
            if required_ids
                .iter()
                .any(|value| !super::is_canonical_id(value))
                || entry
                    .approval_id
                    .as_deref()
                    .is_some_and(|value| !super::is_canonical_id(value))
                || super::bounded_text(&entry.occurred_at) != entry.occurred_at
                || !is_summary_byte_count(&entry.result_summary)
                || !validate_action_projection(
                    entry.action_id.as_deref(),
                    entry.action_summary.as_deref(),
                    entry.action_hash.as_deref(),
                )
            {
                return Err(AuditError::Corrupt);
            }
        }
        Ok(())
    }
}

pub(super) fn validate_action_projection(
    action_id: Option<&str>,
    action_summary: Option<&str>,
    action_hash: Option<&str>,
) -> bool {
    let present = [
        action_id.is_some(),
        action_summary.is_some(),
        action_hash.is_some(),
    ];
    if present.iter().any(|value| *value != present[0]) {
        return false;
    }
    if action_id.is_some_and(|value| !super::is_canonical_id(value)) {
        return false;
    }
    let Some(summary) = action_summary else {
        return true;
    };
    if summary.is_empty()
        || summary.len() > MAX_ACTION_SUMMARY_BYTES
        || summary.chars().any(char::is_control)
        || normalize_action_summary(summary) != summary
    {
        return false;
    }
    let Some(hash) = action_hash else {
        return false;
    };
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_summary_byte_count(value: &str) -> bool {
    value
        .strip_prefix("summary_bytes=")
        .and_then(|count| count.parse::<usize>().ok().map(|parsed| (count, parsed)))
        .is_some_and(|(count, parsed)| count == parsed.to_string())
}

pub(super) fn hash_current(entry: &AuditEntryV2) -> Result<[u8; 32], AuditError> {
    AuditRecord::V2(entry.clone()).hash()
}

fn hash_without_signature<T>(entry: &T, clear: impl FnOnce(&mut T)) -> Result<[u8; 32], AuditError>
where
    T: Clone + Serialize,
{
    let mut unsigned = entry.clone();
    clear(&mut unsigned);
    let bytes = serde_json::to_vec(&unsigned).map_err(|_| AuditError::InvalidRecord)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}
