use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};

use super::super::{hash_canonical_action, StoreError};

const EXPECTED_V5_SCHEMA_OBJECT_HASHES: &[(&str, &str, &str)] = &[
    (
        "table",
        "observations",
        "ea5206d1c25cbc3fc3a889ad504c7fa5465bb62bb6224ce0626903d1a6296f92",
    ),
    (
        "table",
        "tool_attempts",
        "15ce9d8701bc71b49d25454297cc308250b39a92259f4571e1ed40a2536fd43b",
    ),
    (
        "index",
        "idx_observations_call",
        "ac543f95d91afa92083b12d1ddf82693441127bbe018ee66114e521b883f5ba5",
    ),
    (
        "index",
        "idx_observations_id_call",
        "113010f6cf407cbc7e9ec9fb5351e85a6fa9b4c3e3ab2b165ecead32e915a059",
    ),
    (
        "index",
        "idx_tool_attempts_observation",
        "37398af51b75b8b6de14b6349b39b90d975dcbdd3e868cac5491da029d44475e",
    ),
    (
        "trigger",
        "trg_observations_validate_tool_insert",
        "d181fd622b1e6b72ccec764b22da0406c1657d4525f67b1b077d74d12e46613f",
    ),
    (
        "trigger",
        "trg_tool_attempts_validate_observation_insert",
        "57e5147a2e5cf3231744df077e4c3963e34d30177df4f5446862647c3cb8215f",
    ),
    (
        "trigger",
        "trg_tool_attempts_validate_observation_update",
        "92eeb02808900256d4bf36021ab041fa9d4cbac59f08aef74e31fd4d415e2e9f",
    ),
    (
        "trigger",
        "trg_observations_no_update",
        "4bcc92e79ee07c842c193e282cc4ef6564ec19e125c21d546be58bde1a632e0a",
    ),
    (
        "trigger",
        "trg_observations_no_delete",
        "93c96415b881eeff639407c7541581ad421d7514eb9be808d4c2a936bd2ab513",
    ),
];

pub(super) fn verify_schema_shape(connection: &Connection) -> Result<(), StoreError> {
    let (count, minimum, maximum): (u32, Option<u32>, Option<u32>) = connection.query_row(
        "SELECT COUNT(*), MIN(version), MAX(version) FROM schema_versions",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if count != super::CURRENT_SCHEMA_VERSION
        || minimum != Some(1)
        || maximum != Some(super::CURRENT_SCHEMA_VERSION)
    {
        return Err(StoreError::Corrupt);
    }
    require_columns(
        connection,
        "actions",
        &[
            "policy_event_id",
            "audit_event_id",
            "audit_sequence",
            "origin_model_call_id",
        ],
    )?;
    require_text_column(connection, "actions", "origin_model_call_id")?;
    require_model_phase_schema(connection)?;
    require_observation_schema(connection)?;
    require_columns(
        connection,
        "approvals",
        &[
            "approval_id",
            "run_id",
            "action_id",
            "owner_actor_id",
            "state",
            "action_hash",
            "action_summary",
            "workspace_identity",
            "policy_snapshot_hash",
            "config_snapshot_hash",
            "created_at_unix",
            "expires_at_unix",
            "capability_nonce_hash",
            "approval_event_id",
            "row_version",
        ],
    )?;
    for index in [
        "idx_actions_policy_event",
        "idx_actions_audit_event",
        "idx_audit_file_sequence",
        "idx_approvals_run_owner",
        "idx_approvals_state_expiry",
        "idx_approvals_nonce_hash",
    ] {
        let present: bool = connection
            .query_row(
                "SELECT 1 FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                params![index],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !present {
            return Err(StoreError::Corrupt);
        }
    }
    let mut foreign_keys = connection.prepare("PRAGMA foreign_key_check")?;
    if foreign_keys.query([])?.next()?.is_some() {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}

fn require_columns(
    connection: &Connection,
    table: &str,
    required: &[&str],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<HashSet<_>, _>>()?;
    if required.iter().all(|column| columns.contains(*column)) {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_text_column(
    connection: &Connection,
    table: &str,
    required: &str,
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == required
            && row.get::<_, String>(2)?.eq_ignore_ascii_case("TEXT")
        {
            return Ok(());
        }
    }
    Err(StoreError::Corrupt)
}

fn require_model_phase_schema(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA table_info(steps)")?;
    let mut rows = statement.query([])?;
    let mut column_is_valid = false;
    while let Some(row) = rows.next()? {
        let name = row.get::<_, String>(1)?;
        if name != "model_phase" {
            continue;
        }
        let declared_type = row.get::<_, String>(2)?;
        let not_null = row.get::<_, u32>(3)?;
        let default_value = row.get::<_, Option<String>>(4)?;
        column_is_valid = declared_type.eq_ignore_ascii_case("TEXT")
            && not_null == 1
            && default_value.as_deref() == Some("'not_started'");
        break;
    }
    if !column_is_valid {
        return Err(StoreError::Corrupt);
    }

    let schema_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'steps'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact_schema = compact_sql(&schema_sql);
    if compact_schema.contains("check(model_phasein('not_started','running','completed'))") {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_observation_schema(connection: &Connection) -> Result<(), StoreError> {
    require_columns(
        connection,
        "observations",
        &[
            "observation_id",
            "run_id",
            "step_id",
            "call_id",
            "kind",
            "sanitized_payload",
            "fingerprint",
            "created_at",
            "outcome",
        ],
    )?;
    require_columns(
        connection,
        "tool_attempts",
        &["call_id", "action_id", "observation_id"],
    )?;

    let mut statement = connection.prepare("PRAGMA table_info(observations)")?;
    let mut rows = statement.query([])?;
    let mut outcome_is_valid = false;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? != "outcome" {
            continue;
        }
        outcome_is_valid = row.get::<_, String>(2)?.eq_ignore_ascii_case("TEXT")
            && row.get::<_, u32>(3)? == 1
            && row.get::<_, Option<String>>(4)?.as_deref() == Some("'completed'");
        break;
    }
    if !outcome_is_valid {
        return Err(StoreError::Corrupt);
    }

    let schema_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'observations'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact_schema = schema_sql
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if !compact_schema.contains("check(outcomein('completed','failed','absent'))") {
        return Err(StoreError::Corrupt);
    }

    require_unique_index(
        connection,
        "observations",
        "idx_observations_call",
        &["call_id"],
        false,
    )?;
    require_unique_index(
        connection,
        "observations",
        "idx_observations_id_call",
        &["observation_id", "call_id"],
        false,
    )?;
    require_unique_index(
        connection,
        "tool_attempts",
        "idx_tool_attempts_observation",
        &["observation_id"],
        true,
    )?;
    require_tool_attempt_observation_fk(connection)?;

    let attempt_schema = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'tool_attempts'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact_attempt_schema = compact_sql(&attempt_schema);
    for required in [
        "foreignkey(observation_id,call_id)referencesobservations(observation_id,call_id)",
        "statein('created','started')andobservation_idisnull",
        "statein('completed','failed','cancelled','interrupted')andobservation_idisnotnull",
    ] {
        if !compact_attempt_schema.contains(required) {
            return Err(StoreError::Corrupt);
        }
    }

    require_trigger_fragments(
        connection,
        "trg_observations_validate_tool_insert",
        &[
            "beforeinsertonobservations",
            "json_valid(new.sanitized_payload)!=1",
            "length(cast(new.sanitized_payloadasblob))>65536",
            "attempt.call_id=new.call_id",
            "action.call_id=attempt.call_id",
            "action.run_id=new.run_id",
            "action.step_id=new.step_id",
            "step.action_id=action.action_id",
            "new.kind='tool.absent'andnew.outcome!='absent'",
        ],
    )?;
    let binding_fragments = [
        "observation.observation_id=new.observation_id",
        "observation.call_id=new.call_id",
        "observation.run_id=action.run_id",
        "observation.step_id=action.step_id",
        "action.call_id=new.call_id",
        "step.action_id=action.action_id",
        "new.state='completed'andobservation.kind='tool.completed'",
        "new.state='failed'andobservation.kind='tool.failed'",
    ];
    require_trigger_fragments(
        connection,
        "trg_tool_attempts_validate_observation_insert",
        &binding_fragments,
    )?;
    require_trigger_fragments(
        connection,
        "trg_tool_attempts_validate_observation_update",
        &binding_fragments,
    )?;
    require_trigger_fragments(
        connection,
        "trg_observations_no_update",
        &[
            "beforeupdateonobservations",
            "raise(abort,'durableobservationsareappend-only')",
        ],
    )?;
    require_trigger_fragments(
        connection,
        "trg_observations_no_delete",
        &[
            "beforedeleteonobservations",
            "raise(abort,'durableobservationsareappend-only')",
        ],
    )?;
    require_schema_object_hashes(connection)?;
    require_observation_row_integrity(connection)
}

fn compact_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn require_unique_index(
    connection: &Connection,
    table: &str,
    index: &str,
    expected_columns: &[&str],
    expected_partial: bool,
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(&format!("PRAGMA index_list({table})"))?;
    let mut rows = statement.query([])?;
    let mut valid_metadata = false;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? != index {
            continue;
        }
        valid_metadata = row.get::<_, u32>(2)? == 1
            && row.get::<_, String>(3)? == "c"
            && (row.get::<_, u32>(4)? == 1) == expected_partial;
        break;
    }
    if !valid_metadata {
        return Err(StoreError::Corrupt);
    }

    let index_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = ?1",
            params![index],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact_index_sql = compact_sql(&index_sql);
    if contains_sql_comment(&index_sql)
        || (expected_partial && !compact_index_sql.contains("whereobservation_idisnotnull"))
    {
        return Err(StoreError::Corrupt);
    }

    let mut statement = connection.prepare(&format!("PRAGMA index_info({index})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(2))?
        .collect::<Result<Vec<_>, _>>()?;
    if columns
        .iter()
        .map(String::as_str)
        .eq(expected_columns.iter().copied())
    {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_tool_attempt_observation_fk(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA foreign_key_list(tool_attempts)")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut observation_rows = rows
        .into_iter()
        .filter(|(_, _, table, _, _)| table == "observations")
        .collect::<Vec<_>>();
    observation_rows.sort_by_key(|(_, sequence, _, _, _)| *sequence);
    let valid = matches!(
        observation_rows.as_slice(),
        [
            (first_id, 0, _, first_from, first_to),
            (second_id, 1, _, second_from, second_to),
        ] if first_id == second_id
            && first_from == "observation_id"
            && first_to == "observation_id"
            && second_from == "call_id"
            && second_to == "call_id"
    );
    if valid {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_trigger_fragments(
    connection: &Connection,
    trigger: &str,
    required_fragments: &[&str],
) -> Result<(), StoreError> {
    let sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'trigger' AND name = ?1",
            params![trigger],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    if contains_sql_comment(&sql) {
        return Err(StoreError::Corrupt);
    }
    let compact = compact_sql(&sql);
    if compact.contains("raise(abort,")
        && required_fragments
            .iter()
            .all(|fragment| compact.contains(fragment))
    {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn contains_sql_comment(sql: &str) -> bool {
    sql.contains("--") || sql.contains("/*")
}

fn require_schema_object_hashes(connection: &Connection) -> Result<(), StoreError> {
    for (kind, name, expected) in EXPECTED_V5_SCHEMA_OBJECT_HASHES {
        let sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2",
                params![kind, name],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::Corrupt)?;
        if contains_sql_comment(&sql) || hash_canonical_action(&compact_sql(&sql)) != *expected {
            return Err(StoreError::Corrupt);
        }
    }
    Ok(())
}

fn require_observation_row_integrity(connection: &Connection) -> Result<(), StoreError> {
    let invalid_attempt: bool = connection
        .query_row(
            "SELECT 1
             FROM tool_attempts AS attempt
             JOIN actions AS action ON action.action_id = attempt.action_id
             LEFT JOIN steps AS step
               ON step.run_id = action.run_id AND step.step_id = action.step_id
             LEFT JOIN observations AS observation
               ON observation.observation_id = attempt.observation_id
              AND observation.call_id = attempt.call_id
             WHERE attempt.call_id != action.call_id
                OR step.step_id IS NULL
                OR step.action_id IS NULL
                OR step.action_id != action.action_id
                OR (attempt.state IN ('created', 'started')
                    AND attempt.observation_id IS NOT NULL)
                OR (attempt.state IN ('completed', 'failed', 'cancelled', 'interrupted')
                    AND (
                      attempt.observation_id IS NULL
                      OR observation.observation_id IS NULL
                      OR observation.run_id != action.run_id
                      OR observation.step_id != action.step_id
                      OR NOT (
                        (observation.kind = 'tool.absent' AND observation.outcome = 'absent')
                        OR (attempt.state = 'completed'
                            AND observation.kind = 'tool.completed'
                            AND observation.outcome = 'completed')
                        OR (attempt.state = 'failed'
                            AND observation.kind = 'tool.failed'
                            AND observation.outcome = 'failed')
                      )
                    ))
             LIMIT 1",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if invalid_attempt {
        return Err(StoreError::Corrupt);
    }

    let orphan_tool_observation: bool = connection
        .query_row(
            "SELECT 1
             FROM observations AS observation
             LEFT JOIN tool_attempts AS attempt
               ON attempt.observation_id = observation.observation_id
              AND attempt.call_id = observation.call_id
             WHERE observation.kind LIKE 'tool.%' AND attempt.call_id IS NULL
             LIMIT 1",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if orphan_tool_observation {
        Err(StoreError::Corrupt)
    } else {
        Ok(())
    }
}
