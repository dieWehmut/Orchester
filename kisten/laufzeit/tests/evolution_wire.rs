mod evolution_support;

use evolution_support::*;
use orchester_laufzeit::harness::evolution::CandidateState;
use sha2::{Digest, Sha256};

fn evaluation_id_for(candidate_id: &str, snapshots: &[&str]) -> String {
    let mut bytes = Vec::new();
    for field in std::iter::once("orchester-evaluation-id-v1")
        .chain(std::iter::once(candidate_id))
        .chain(snapshots.iter().copied())
    {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn candidate_wire_round_trip_recomputes_id_and_rejects_tampering() {
    let candidate = manifest(
        scope(),
        CandidateKind::ToolPolicy,
        Some(parent_revision()),
        artifact('a', 12, "json"),
        CREATED_AT,
        EXPIRES_AT,
    );
    let encoded = serde_json::to_string(&candidate).unwrap();
    let decoded: CandidateManifestV1 = serde_json::from_str(&encoded).unwrap();
    assert_eq!(candidate, decoded);

    let tampered_id = encoded.replace(candidate.candidate_id().as_str(), &"b".repeat(64));
    assert!(
        serde_json::from_str::<CandidateManifestV1>(&tampered_id)
            .unwrap_err()
            .to_string()
            .contains("evolution identity is corrupt")
    );

    let tampered_payload = encoded.replace(&"a".repeat(64), &"c".repeat(64));
    assert!(
        serde_json::from_str::<CandidateManifestV1>(&tampered_payload)
            .unwrap_err()
            .to_string()
            .contains("evolution identity is corrupt")
    );

    let unknown_schema = encoded.replace("\"schema_version\":1", "\"schema_version\":2");
    assert!(
        serde_json::from_str::<CandidateManifestV1>(&unknown_schema)
            .unwrap_err()
            .to_string()
            .contains("evolution schema is unsupported")
    );

    let unknown_field = encoded.replacen('{', "{\"unexpected\":true,", 1);
    assert!(
        serde_json::from_str::<CandidateManifestV1>(&unknown_field)
            .unwrap_err()
            .to_string()
            .contains("evolution input is invalid")
    );
}

#[test]
fn evaluation_wire_round_trip_recomputes_id_and_rejects_tampering() {
    let candidate = base_manifest();
    let key = evaluation(&candidate, snapshots());
    let encoded = serde_json::to_string(&key).unwrap();
    let decoded: EvaluationKey = serde_json::from_str(&encoded).unwrap();
    assert_eq!(key, decoded);

    let tampered_id = encoded.replace(key.evaluation_id().as_str(), &"c".repeat(64));
    assert!(
        serde_json::from_str::<EvaluationKey>(&tampered_id)
            .unwrap_err()
            .to_string()
            .contains("evolution identity is corrupt")
    );

    let tampered_snapshot = encoded.replacen(&"1".repeat(64), &"a".repeat(64), 1);
    assert!(
        serde_json::from_str::<EvaluationKey>(&tampered_snapshot)
            .unwrap_err()
            .to_string()
            .contains("evolution identity is corrupt")
    );

    let mut unknown_schema = serde_json::to_value(&key).unwrap();
    unknown_schema["schema_version"] = 2.into();
    assert!(
        serde_json::from_value::<EvaluationKey>(unknown_schema)
            .unwrap_err()
            .to_string()
            .contains("evolution schema is unsupported")
    );

    let unknown_field = encoded.replacen('{', "{\"unexpected\":true,", 1);
    assert!(
        serde_json::from_str::<EvaluationKey>(&unknown_field)
            .unwrap_err()
            .to_string()
            .contains("evolution input is invalid")
    );
}

#[test]
fn evaluation_wire_rejects_a_forged_but_self_consistent_candidate_id() {
    let candidate = base_manifest();
    let key = evaluation(&candidate, snapshots());
    let mut wire = serde_json::to_value(&key).unwrap();
    let forged_candidate = "b".repeat(64);
    let snapshot_values = [
        "corpus_hash",
        "evaluator_hash",
        "config_hash",
        "policy_hash",
        "catalog_hash",
        "environment_hash",
    ]
    .map(|field| wire[field].as_str().unwrap().to_owned());
    let snapshot_refs = snapshot_values
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    wire["candidate"]["candidate_id"] = forged_candidate.clone().into();
    wire["evaluation_id"] = evaluation_id_for(&forged_candidate, &snapshot_refs).into();

    let error = serde_json::from_value::<EvaluationKey>(wire).unwrap_err();
    assert!(error.to_string().contains("evolution input is invalid"));
}

#[test]
fn wire_decode_errors_never_echo_attacker_controlled_names_or_variants() {
    const CANARY: &str = "sensitive-canary-do-not-log-42";

    let candidate = base_manifest();
    let mut unknown_candidate_field = serde_json::to_value(&candidate).unwrap();
    unknown_candidate_field
        .as_object_mut()
        .unwrap()
        .insert(CANARY.to_owned(), true.into());
    assert_decode_error_redacts::<CandidateManifestV1>(unknown_candidate_field, CANARY);

    let mut unknown_evaluation_field =
        serde_json::to_value(evaluation(&candidate, snapshots())).unwrap();
    unknown_evaluation_field
        .as_object_mut()
        .unwrap()
        .insert(CANARY.to_owned(), true.into());
    assert_decode_error_redacts::<EvaluationKey>(unknown_evaluation_field, CANARY);

    let mut unknown_kind = serde_json::to_value(&candidate).unwrap();
    unknown_kind["kind"] = CANARY.into();
    assert_decode_error_redacts::<CandidateManifestV1>(unknown_kind, CANARY);
    assert_decode_error_redacts::<CandidateKind>(CANARY.into(), CANARY);
    assert_decode_error_redacts::<CandidateState>(CANARY.into(), CANARY);
}

fn assert_decode_error_redacts<T>(value: serde_json::Value, canary: &str)
where
    T: serde::de::DeserializeOwned,
{
    let error = serde_json::from_value::<T>(value)
        .err()
        .expect("attacker-controlled wire value must be rejected");
    assert!(!error.to_string().contains(canary));
    assert!(!format!("{error:?}").contains(canary));
}
