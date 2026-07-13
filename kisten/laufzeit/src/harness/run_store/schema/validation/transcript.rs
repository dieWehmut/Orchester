use rusqlite::{params, Connection, OptionalExtension};

use super::{compact_sql, contains_sql_comment, require_columns, require_trigger_fragments};
use crate::harness::run_store::StoreError;

pub(super) fn verify_schema(connection: &Connection) -> Result<(), StoreError> {
    require_columns(
        connection,
        "transcript_records",
        &[
            "run_id",
            "ordinal",
            "kind",
            "call_id",
            "wire_json",
            "record_hash",
            "created_at",
        ],
    )?;
    require_column_types(connection)?;
    require_table_constraints(connection)?;
    require_primary_key(connection, &["run_id", "ordinal"])?;
    require_non_unique_index(
        connection,
        "idx_transcript_records_run_ordinal",
        &["run_id", "ordinal"],
    )?;
    require_trigger_fragments(
        connection,
        "trg_transcript_records_no_update",
        &[
            "beforeupdateontranscript_records",
            "raise(abort,'durabletranscriptrecordsareappend-only')",
        ],
    )?;
    require_trigger_fragments(
        connection,
        "trg_transcript_records_no_delete",
        &[
            "beforedeleteontranscript_records",
            "raise(abort,'durabletranscriptrecordsareappend-only')",
        ],
    )
}

fn require_column_types(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA table_info(transcript_records)")?;
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
        ("ordinal", "INTEGER", true),
        ("kind", "TEXT", true),
        ("call_id", "TEXT", false),
        ("wire_json", "TEXT", true),
        ("record_hash", "TEXT", true),
        ("created_at", "TEXT", true),
    ] {
        let valid = columns
            .iter()
            .any(|(actual_name, declared_type, not_null)| {
                actual_name == name
                    && declared_type.eq_ignore_ascii_case(expected_type)
                    && *not_null == expected_not_null
            });
        if !valid {
            return Err(StoreError::Corrupt);
        }
    }
    Ok(())
}

fn require_table_constraints(connection: &Connection) -> Result<(), StoreError> {
    let schema_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'transcript_records'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact_schema = compact_sql(&schema_sql);
    for required in [
        "check(ordinal>=1)",
        "check(kindin('system','user','assistant','tool_call','tool_result','opaque'))",
        "json_valid(wire_json)=1",
        "length(cast(wire_jsonasblob))<=65536",
        "length(record_hash)=64",
        "record_hashnotglob'*[^0-9a-f]*'",
        "primarykey(run_id,ordinal)",
    ] {
        if !compact_schema.contains(required) {
            return Err(StoreError::Corrupt);
        }
    }
    Ok(())
}

fn require_primary_key(
    connection: &Connection,
    expected_columns: &[&str],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA table_info(transcript_records)")?;
    let mut columns = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, u32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    columns.retain(|(_, position)| *position > 0);
    columns.sort_by_key(|(_, position)| *position);
    if columns
        .iter()
        .map(|(name, _)| name.as_str())
        .eq(expected_columns.iter().copied())
    {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_non_unique_index(
    connection: &Connection,
    index: &str,
    expected_columns: &[&str],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA index_list(transcript_records)")?;
    let mut rows = statement.query([])?;
    let mut valid_metadata = false;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? != index {
            continue;
        }
        valid_metadata = row.get::<_, u32>(2)? == 0
            && row.get::<_, String>(3)? == "c"
            && row.get::<_, u32>(4)? == 0;
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
    if contains_sql_comment(&index_sql) {
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
