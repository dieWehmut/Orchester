mod evolution_support;

use std::collections::BTreeSet;

use evolution_support::*;

#[test]
fn equal_inputs_have_stable_lowercase_hex_ids() {
    let left = base_manifest();
    let right = base_manifest();
    assert_eq!(left.candidate_id(), right.candidate_id());

    let left_key = evaluation(&left, snapshots());
    let right_key = evaluation(&right, snapshots());
    assert_eq!(left_key.evaluation_id(), right_key.evaluation_id());

    let revision = RevisionId::derive(left.candidate_id(), left_key.evaluation_id(), None);
    for value in [
        left.candidate_id().as_str(),
        left_key.evaluation_id().as_str(),
        revision.as_str(),
    ] {
        assert_lower_hex(value);
    }
}

#[test]
fn identity_encoding_matches_known_sha256_vectors() {
    let candidate = base_manifest();
    let key = evaluation(&candidate, snapshots());
    let revision = RevisionId::derive(candidate.candidate_id(), key.evaluation_id(), None);

    assert_eq!(
        candidate.candidate_id().as_str(),
        "d02a5e7b219f652bb30e625e9fb3db48e74220755d2c2608a405571062cc14ea"
    );
    assert_eq!(
        key.evaluation_id().as_str(),
        "f5bedede46a1faa8a465eeaeae9bce3879eac59e7ef5acd8bf165f35cdba4484"
    );
    assert_eq!(
        revision.as_str(),
        "3ee3399c0c0c496ef7e7d5f5b7a2cdb8dd584306f625463e278a73443d731a19"
    );
}

#[test]
fn length_prefixes_prevent_adjacent_field_ambiguity() {
    let left = manifest(
        EvolutionScope::try_new("a", "bc").unwrap(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "json"),
        CREATED_AT,
        EXPIRES_AT,
    );
    let right = manifest(
        EvolutionScope::try_new("ab", "c").unwrap(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "json"),
        CREATED_AT,
        EXPIRES_AT,
    );
    assert_ne!(left.candidate_id(), right.candidate_id());
}

#[test]
fn equivalent_fractional_timestamps_have_one_candidate_identity() {
    let concise = manifest(
        scope(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "json"),
        "2026-07-15T10:00:00.9Z",
        "2026-07-15T10:00:01Z",
    );
    let padded = manifest(
        scope(),
        CandidateKind::Strategy,
        None,
        artifact('a', 12, "json"),
        "2026-07-15T10:00:00.900000000Z",
        "2026-07-15T10:00:01.000Z",
    );

    assert_eq!(concise.candidate_id(), padded.candidate_id());
    assert_eq!(concise.created_at(), "2026-07-15T10:00:00.9Z");
    assert_eq!(padded.created_at(), concise.created_at());
    assert_eq!(padded.expires_at(), "2026-07-15T10:00:01Z");
}

#[test]
fn every_candidate_identity_field_changes_the_candidate_id() {
    let base = base_manifest();
    let variants = [
        manifest(
            EvolutionScope::try_new("project-b", "owner-a").unwrap(),
            CandidateKind::Strategy,
            None,
            artifact('a', 12, "json"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            EvolutionScope::try_new("project-a", "owner-b").unwrap(),
            CandidateKind::Strategy,
            None,
            artifact('a', 12, "json"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Prompt,
            None,
            artifact('a', 12, "json"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Strategy,
            Some(parent_revision()),
            artifact('a', 12, "json"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Strategy,
            None,
            artifact('b', 12, "json"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Strategy,
            None,
            artifact('a', 13, "json"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Strategy,
            None,
            artifact('a', 12, "toml"),
            CREATED_AT,
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Strategy,
            None,
            artifact('a', 12, "json"),
            "2026-07-15T11:00:00Z",
            EXPIRES_AT,
        ),
        manifest(
            scope(),
            CandidateKind::Strategy,
            None,
            artifact('a', 12, "json"),
            CREATED_AT,
            "2026-07-17T10:00:00Z",
        ),
    ];

    let mut ids = BTreeSet::from([base.candidate_id().as_str().to_owned()]);
    for variant in variants {
        assert_ne!(base.candidate_id(), variant.candidate_id());
        ids.insert(variant.candidate_id().as_str().to_owned());
    }
    assert_eq!(ids.len(), 10);
}

#[test]
fn every_evaluation_snapshot_changes_the_evaluation_id() {
    let candidate = base_manifest();
    let base = evaluation(&candidate, snapshots());
    for index in 0..6 {
        let mut fixture = SnapshotFixture::complete();
        let replacement = Some(digest(char::from(b'a' + index as u8)));
        match index {
            0 => fixture.corpus = replacement,
            1 => fixture.evaluator = replacement,
            2 => fixture.config = replacement,
            3 => fixture.policy = replacement,
            4 => fixture.catalog = replacement,
            5 => fixture.environment = replacement,
            _ => unreachable!(),
        }
        let changed = evaluation(&candidate, fixture.input());
        assert_ne!(base.evaluation_id(), changed.evaluation_id());
    }
}

#[test]
fn evaluation_snapshots_preserve_names_end_to_end() {
    let candidate = base_manifest();
    let key = evaluation(&candidate, snapshots());

    assert_eq!(key.snapshots().corpus(), &digest('1'));
    assert_eq!(key.snapshots().evaluator(), &digest('2'));
    assert_eq!(key.snapshots().config(), &digest('3'));
    assert_eq!(key.snapshots().policy(), &digest('4'));
    assert_eq!(key.snapshots().catalog(), &digest('5'));
    assert_eq!(key.snapshots().environment(), &digest('6'));
}

#[test]
fn candidate_evaluation_and_revision_domains_are_separate() {
    let candidate = base_manifest();
    let key = evaluation(&candidate, snapshots());
    let revision = RevisionId::derive(candidate.candidate_id(), key.evaluation_id(), None);
    assert_ne!(
        candidate.candidate_id().as_str(),
        key.evaluation_id().as_str()
    );
    assert_ne!(revision.as_str(), candidate.candidate_id().as_str());
    assert_ne!(revision.as_str(), key.evaluation_id().as_str());
}

#[test]
fn revision_id_binds_candidate_evaluation_and_parent_independently() {
    let first = base_manifest();
    let second = manifest(
        scope(),
        CandidateKind::Prompt,
        None,
        artifact('a', 12, "json"),
        CREATED_AT,
        EXPIRES_AT,
    );
    let first_evaluation = evaluation(&first, snapshots());
    let second_evaluation = evaluation(&second, snapshots());
    let base = RevisionId::derive(first.candidate_id(), first_evaluation.evaluation_id(), None);

    assert_ne!(
        base,
        RevisionId::derive(
            second.candidate_id(),
            first_evaluation.evaluation_id(),
            None
        )
    );
    assert_ne!(
        base,
        RevisionId::derive(
            first.candidate_id(),
            second_evaluation.evaluation_id(),
            None
        )
    );
    assert_ne!(
        base,
        RevisionId::derive(
            first.candidate_id(),
            first_evaluation.evaluation_id(),
            Some(&parent_revision()),
        )
    );
}
