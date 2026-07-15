mod evolution_support;

use evolution_support::*;
use orchester_laufzeit::harness::evolution::{CandidateState, EvolutionError, can_transition};

const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

#[test]
fn malformed_values_fail_without_echoing_input() {
    let invalid_digests = [
        String::new(),
        "ABC".to_owned(),
        "0".to_owned(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
    ];
    for value in invalid_digests {
        let error = EvolutionDigest::try_from(value.as_str()).unwrap_err();
        assert_eq!(error, EvolutionError::InvalidDigest);
        if !value.is_empty() {
            assert!(!format!("{error:?}").contains(&value));
        }
    }

    assert_eq!(
        EvolutionScope::try_new("", "owner").unwrap_err(),
        EvolutionError::InvalidInput
    );
    assert_eq!(
        EvolutionScope::try_new("project\u{1b}[2J", "owner").unwrap_err(),
        EvolutionError::InvalidInput
    );
    for format_character in ['\u{0600}', '\u{206a}', '\u{e0001}'] {
        assert_eq!(
            EvolutionScope::try_new(format!("project{format_character}"), "owner").unwrap_err(),
            EvolutionError::InvalidInput
        );
        assert_eq!(
            ArtifactRef::try_new(digest('a'), 12, format!("json{format_character}")).unwrap_err(),
            EvolutionError::InvalidInput
        );
    }
    assert_eq!(
        ArtifactRef::try_new(digest('a'), 0, "json").unwrap_err(),
        EvolutionError::InvalidInput
    );
    assert_eq!(
        ArtifactRef::try_new(digest('a'), 12, "").unwrap_err(),
        EvolutionError::InvalidInput
    );
}

#[test]
fn text_and_artifact_limits_are_enforced_in_utf8_bytes() {
    assert!(EvolutionScope::try_new("p".repeat(256), "o".repeat(256)).is_ok());
    assert_eq!(
        EvolutionScope::try_new("p".repeat(257), "owner").unwrap_err(),
        EvolutionError::InvalidInput
    );
    assert_eq!(
        EvolutionScope::try_new("project", "o".repeat(257)).unwrap_err(),
        EvolutionError::InvalidInput
    );
    assert!(EvolutionScope::try_new("é".repeat(128), "owner").is_ok());
    assert_eq!(
        EvolutionScope::try_new("é".repeat(129), "owner").unwrap_err(),
        EvolutionError::InvalidInput
    );

    assert!(ArtifactRef::try_new(digest('a'), MAX_ARTIFACT_BYTES, "f".repeat(64)).is_ok());
    assert_eq!(
        ArtifactRef::try_new(digest('a'), MAX_ARTIFACT_BYTES + 1, "json").unwrap_err(),
        EvolutionError::InvalidInput
    );
    assert_eq!(
        ArtifactRef::try_new(digest('a'), 12, "f".repeat(65)).unwrap_err(),
        EvolutionError::InvalidInput
    );
}

#[test]
fn identifiers_reject_ambiguous_whitespace_and_line_separators() {
    for project in [
        " ",
        "\u{2003}",
        " project",
        "project ",
        "project\u{2028}next",
        "project\u{2029}next",
    ] {
        assert_eq!(
            EvolutionScope::try_new(project, "owner").unwrap_err(),
            EvolutionError::InvalidInput
        );
    }
}

#[test]
fn timestamp_and_expiry_bounds_fail_closed() {
    let cases = [
        ("2026-02-30T10:00:00Z", "2026-03-01T10:00:00Z"),
        ("2026-07-15T10:00:00", EXPIRES_AT),
        (CREATED_AT, CREATED_AT),
        (CREATED_AT, "2026-07-14T10:00:00Z"),
    ];
    for (created, expires) in cases {
        let error = CandidateManifestV1::try_new(
            scope(),
            CandidateKind::Strategy,
            None,
            artifact('a', 12, "json"),
            created,
            expires,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            EvolutionError::InvalidTimestamp | EvolutionError::InvalidExpiry
        ));
    }

    let fractional_equal = CandidateManifestV1::try_new(
        scope(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "json"),
        "2026-07-15T10:00:00.90Z",
        "2026-07-15T10:00:00.9Z",
    )
    .unwrap_err();
    assert_eq!(fractional_equal, EvolutionError::InvalidExpiry);
}

#[test]
fn missing_evaluation_snapshots_are_rejected_individually() {
    let candidate = base_manifest();
    for index in 0..6 {
        let mut fixture = SnapshotFixture::complete();
        match index {
            0 => fixture.corpus = None,
            1 => fixture.evaluator = None,
            2 => fixture.config = None,
            3 => fixture.policy = None,
            4 => fixture.catalog = None,
            5 => fixture.environment = None,
            _ => unreachable!(),
        }
        let error = EvaluationKey::try_new(candidate.clone(), fixture.input()).unwrap_err();
        assert_eq!(error, EvolutionError::MissingSnapshot);
    }
}

#[test]
fn debug_views_redact_identity_and_user_controlled_text() {
    let digest_value = "a".repeat(64);
    let candidate = manifest(
        EvolutionScope::try_new("project-private-name", "owner-private-name").unwrap(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "private-format-name"),
        CREATED_AT,
        EXPIRES_AT,
    );
    let snapshot_set = snapshots();
    let key = evaluation(&candidate, snapshot_set.clone());
    let revision = RevisionId::derive(candidate.candidate_id(), key.evaluation_id(), None);
    let debug = format!(
        "{:?} {:?} {:?} {:?} {:?}",
        digest('a'),
        candidate,
        snapshot_set,
        key,
        revision
    );
    for sensitive in [
        digest_value.as_str(),
        "project-private-name",
        "owner-private-name",
        "private-format-name",
        candidate.candidate_id().as_str(),
        key.evaluation_id().as_str(),
        revision.as_str(),
    ] {
        assert!(!debug.contains(sensitive));
    }
}

#[test]
fn candidate_kind_has_exactly_three_wire_variants() {
    fn exhaustive(kind: CandidateKind) -> &'static str {
        match kind {
            CandidateKind::Strategy => "strategy",
            CandidateKind::Prompt => "prompt",
            CandidateKind::ToolPolicy => "tool_policy",
        }
    }

    let kinds = [
        CandidateKind::Strategy,
        CandidateKind::Prompt,
        CandidateKind::ToolPolicy,
    ];
    assert_eq!(kinds.map(exhaustive), ["strategy", "prompt", "tool_policy"]);
    for (kind, expected) in kinds.into_iter().zip(["strategy", "prompt", "tool_policy"]) {
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{expected}\"")
        );
    }
}

#[test]
fn candidate_state_has_exactly_five_variants_and_a_locked_transition_matrix() {
    fn exhaustive(state: CandidateState) -> usize {
        match state {
            CandidateState::Proposed => 0,
            CandidateState::Evaluating => 1,
            CandidateState::Evaluated => 2,
            CandidateState::Quarantined => 3,
            CandidateState::Expired => 4,
        }
    }

    let states = [
        CandidateState::Proposed,
        CandidateState::Evaluating,
        CandidateState::Evaluated,
        CandidateState::Quarantined,
        CandidateState::Expired,
    ];
    assert_eq!(states.map(exhaustive), [0, 1, 2, 3, 4]);
    for from in states {
        for to in states {
            let expected = matches!(
                (from, to),
                (CandidateState::Proposed, CandidateState::Evaluating)
                    | (CandidateState::Proposed, CandidateState::Quarantined)
                    | (CandidateState::Proposed, CandidateState::Expired)
                    | (CandidateState::Evaluating, CandidateState::Evaluated)
                    | (CandidateState::Evaluating, CandidateState::Quarantined)
                    | (CandidateState::Evaluating, CandidateState::Expired)
                    | (CandidateState::Evaluated, CandidateState::Quarantined)
                    | (CandidateState::Evaluated, CandidateState::Expired)
            );
            assert_eq!(can_transition(from, to), expected, "{from:?} -> {to:?}");
        }
    }
    assert!(serde_json::from_str::<CandidateState>("\"active\"").is_err());
}
