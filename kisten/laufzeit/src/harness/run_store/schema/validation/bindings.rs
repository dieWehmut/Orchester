use rusqlite::{Connection, OptionalExtension};

use super::{compact_sql, require_columns, require_trigger_fragments};
use crate::harness::run_store::StoreError;

pub(super) fn verify_schema(connection: &Connection) -> Result<(), StoreError> {
    require_columns(
        connection,
        "transcript_bindings",
        &[
            "run_id",
            "event_sequence",
            "phase",
            "first_ordinal",
            "last_ordinal",
            "record_count",
        ],
    )?;
    require_column_types(connection)?;
    require_table_constraints(connection)?;
    require_primary_key(connection)?;
    require_index(connection)?;
    require_trigger_fragments(
        connection,
        "trg_transcript_bindings_no_update",
        &[
            "beforeupdateontranscript_bindings",
            "raise(abort,'transcriptbindingsareappend-only')",
        ],
    )?;
    require_trigger_fragments(
        connection,
        "trg_transcript_bindings_no_delete",
        &[
            "beforedeleteontranscript_bindings",
            "raise(abort,'transcriptbindingsareappend-only')",
        ],
    )
}

fn require_column_types(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA table_info(transcript_bindings)")?;
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)? == 1,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (name, expected_type, expected_not_null) in [
        ("run_id", "TEXT", true),
        ("event_sequence", "INTEGER", true),
        ("phase", "TEXT", true),
        ("first_ordinal", "INTEGER", false),
        ("last_ordinal", "INTEGER", false),
        ("record_count", "INTEGER", true),
    ] {
        if !columns.iter().any(|(actual, declared, not_null)| {
            actual == name
                && declared.eq_ignore_ascii_case(expected_type)
                && *not_null == expected_not_null
        }) {
            return Err(StoreError::Corrupt);
        }
    }
    Ok(())
}

fn require_table_constraints(connection: &Connection) -> Result<(), StoreError> {
    let schema_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema
             WHERE type = 'table' AND name = 'transcript_bindings'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact = compact_sql(&schema_sql);
    for required in [
        "check(event_sequence>=1)",
        "check(phasein('model_request','model_response','action','tool_result'))",
        "check(record_count>=0)",
        "foreignkey(run_id,event_sequence)referencesevents(run_id,sequence)",
        "foreignkey(run_id,first_ordinal)referencestranscript_records(run_id,ordinal)",
        "foreignkey(run_id,last_ordinal)referencestranscript_records(run_id,ordinal)",
        "primarykey(run_id,event_sequence,phase)",
        "check(first_ordinalisnullorfirst_ordinal>=1)",
        "check(last_ordinalisnullorlast_ordinal>=1)",
        "check((record_count=0andfirst_ordinalisnullandlast_ordinalisnull)",
    ] {
        if !compact.contains(required) {
            return Err(StoreError::Corrupt);
        }
    }
    Ok(())
}

fn require_primary_key(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA table_info(transcript_bindings)")?;
    let mut columns = statement
        .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, u32>(5)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    columns.retain(|(_, position)| *position > 0);
    columns.sort_by_key(|(_, position)| *position);
    if columns
        .iter()
        .map(|(name, _)| name.as_str())
        .eq(["run_id", "event_sequence", "phase"])
    {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_index(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA index_list(transcript_bindings)")?;
    let mut rows = statement.query([])?;
    let mut found = false;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == "idx_transcript_bindings_run_first" {
            found = row.get::<_, u32>(2)? == 0
                && row.get::<_, String>(3)? == "c"
                && row.get::<_, u32>(4)? == 0;
            break;
        }
    }
    if !found {
        return Err(StoreError::Corrupt);
    }
    let sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'index'
             AND name = 'idx_transcript_bindings_run_first'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    if compact_sql(&sql).contains("ontranscript_bindings(run_id,first_ordinal)") {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}
