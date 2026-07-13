use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use super::StoreError;

mod validation;

use validation::verify_schema_shape;

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../../../migrations/0001_state.sql")),
    (
        2,
        include_str!("../../../migrations/0002_approval_barrier.sql"),
    ),
    (3, include_str!("../../../migrations/0003_model_phase.sql")),
    (
        4,
        include_str!("../../../migrations/0004_action_model_binding.sql"),
    ),
    (
        5,
        include_str!("../../../migrations/0005_observation_links.sql"),
    ),
    (
        6,
        include_str!("../../../migrations/0006_transcript_records.sql"),
    ),
];

pub(super) const CURRENT_SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub(super) fn apply_migrations(connection: &mut Connection) -> Result<(), StoreError> {
    validate_registry()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut version = inspect_version(&transaction)?;

    if version == 0 {
        require_empty_schema(&transaction)?;
        apply_next(&transaction, 1)?;
        version = 1;
    } else {
        verify_schema_at(&transaction, version)?;
    }

    while version < CURRENT_SCHEMA_VERSION {
        let next = version.checked_add(1).ok_or(StoreError::Corrupt)?;
        apply_next(&transaction, next)?;
        version = next;
    }

    verify_version_markers(&transaction, CURRENT_SCHEMA_VERSION)?;
    verify_schema_shape(&transaction, CURRENT_SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn validate_registry() -> Result<(), StoreError> {
    let valid = MIGRATIONS.len() == CURRENT_SCHEMA_VERSION as usize
        && MIGRATIONS
            .iter()
            .enumerate()
            .all(|(index, (version, sql))| {
                *version as usize == index + 1 && !sql.trim().is_empty()
            });
    if valid {
        Ok(())
    } else {
        Err(StoreError::Invariant(
            "state database migration registry is not monotonic".into(),
        ))
    }
}

fn inspect_version(transaction: &Transaction<'_>) -> Result<u32, StoreError> {
    if !schema_object_exists(transaction, "table", "schema_versions")? {
        let user_version =
            transaction.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))?;
        if user_version > CURRENT_SCHEMA_VERSION {
            return Err(StoreError::Invariant(
                "state database schema is newer than this binary".into(),
            ));
        }
        if user_version != 0 {
            return Err(StoreError::Corrupt);
        }
        return Ok(0);
    }
    require_version_table_shape(transaction)?;
    let versions = read_versions(transaction)?;
    let maximum = versions.last().copied().ok_or(StoreError::Corrupt)?;
    if maximum > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::Invariant(
            "state database schema is newer than this binary".into(),
        ));
    }
    verify_version_sequence(&versions, maximum)?;
    verify_user_version(transaction, maximum)?;
    Ok(maximum)
}

fn apply_next(transaction: &Transaction<'_>, next: u32) -> Result<(), StoreError> {
    let (registered, sql) = MIGRATIONS
        .get(next.checked_sub(1).ok_or(StoreError::Corrupt)? as usize)
        .copied()
        .ok_or(StoreError::Corrupt)?;
    if registered != next {
        return Err(StoreError::Invariant(
            "state database migration registry is not monotonic".into(),
        ));
    }
    transaction.execute_batch(sql)?;
    verify_version_markers(transaction, next)?;
    verify_schema_at(transaction, next)
}

fn verify_version_markers(transaction: &Transaction<'_>, expected: u32) -> Result<(), StoreError> {
    require_version_table_shape(transaction)?;
    let versions = read_versions(transaction)?;
    verify_version_sequence(&versions, expected)?;
    verify_user_version(transaction, expected)
}

fn read_versions(transaction: &Transaction<'_>) -> Result<Vec<u32>, StoreError> {
    let mut statement =
        transaction.prepare("SELECT version FROM schema_versions ORDER BY version")?;
    let versions = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)?;
    Ok(versions)
}

fn verify_version_sequence(versions: &[u32], expected: u32) -> Result<(), StoreError> {
    let valid = expected > 0
        && versions.len() == expected as usize
        && versions.iter().copied().eq(1..=expected);
    if valid {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn verify_user_version(transaction: &Transaction<'_>, expected: u32) -> Result<(), StoreError> {
    let user_version =
        transaction.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))?;
    // Migration 1 predates this marker. Migration 2 immediately synchronizes it.
    if user_version == expected || (expected == 1 && user_version == 0) {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_empty_schema(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let existing = transaction
        .query_row(
            "SELECT 1 FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
             LIMIT 1",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if existing {
        Err(StoreError::Corrupt)
    } else {
        Ok(())
    }
}

fn require_version_table_shape(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let mut statement = transaction.prepare("PRAGMA table_info(schema_versions)")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, u32>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let valid = matches!(
        rows.as_slice(),
        [(version, version_type, _, 1), (applied_at, applied_type, 1, 0)]
            if version == "version"
                && version_type.eq_ignore_ascii_case("INTEGER")
                && applied_at == "applied_at"
                && applied_type.eq_ignore_ascii_case("TEXT")
    );
    if valid {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn verify_schema_at(transaction: &Transaction<'_>, version: u32) -> Result<(), StoreError> {
    require_reference_schema(transaction, version)?;
    if version >= 5 {
        verify_schema_shape(transaction, version)?;
    } else {
        require_no_foreign_key_violations(transaction)?;
    }
    Ok(())
}

fn require_reference_schema(transaction: &Transaction<'_>, version: u32) -> Result<(), StoreError> {
    let reference = Connection::open_in_memory()?;
    reference.execute_batch("PRAGMA foreign_keys = ON;")?;
    for (_, sql) in MIGRATIONS.iter().take(version as usize) {
        reference.execute_batch(sql)?;
    }
    if schema_snapshot(transaction)? == schema_snapshot(&reference)? {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn schema_snapshot(connection: &Connection) -> Result<Vec<SchemaObject>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT type, name, sql
         FROM sqlite_schema
         WHERE type IN ('table', 'index', 'trigger')
           AND name NOT LIKE 'sqlite_%'
           AND sql IS NOT NULL
         ORDER BY type, name",
    )?;
    let objects = statement
        .query_map([], |row| {
            Ok(SchemaObject {
                kind: row.get(0)?,
                name: row.get(1)?,
                normalized_sql: normalize_sql(&row.get::<_, String>(2)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(objects)
}

fn normalize_sql(sql: &str) -> String {
    let mut characters = sql.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(character) = characters.next() {
        if character.is_whitespace() {
            continue;
        }
        if let Some(delimiter) = quote_delimiter(character) {
            let mut token = String::from(character);
            while let Some(quoted) = characters.next() {
                token.push(quoted);
                if quoted != delimiter {
                    continue;
                }
                if delimiter != ']' && characters.peek() == Some(&delimiter) {
                    token.push(characters.next().expect("peeked quote is present"));
                } else {
                    break;
                }
            }
            tokens.push(token);
        } else if is_sql_word(character) {
            let mut token = String::from(character.to_ascii_lowercase());
            while characters.peek().is_some_and(|next| is_sql_word(*next)) {
                token.push(
                    characters
                        .next()
                        .expect("peeked word character is present")
                        .to_ascii_lowercase(),
                );
            }
            tokens.push(token);
        } else {
            tokens.push(character.to_string());
        }
    }
    tokens.join("\u{1f}")
}

fn quote_delimiter(character: char) -> Option<char> {
    match character {
        '\'' => Some('\''),
        '"' => Some('"'),
        '`' => Some('`'),
        '[' => Some(']'),
        _ => None,
    }
}

fn is_sql_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
}

#[derive(Debug, PartialEq, Eq)]
struct SchemaObject {
    kind: String,
    name: String,
    normalized_sql: String,
}

fn require_no_foreign_key_violations(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let mut statement = transaction.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        Err(StoreError::Corrupt)
    } else {
        Ok(())
    }
}

fn schema_object_exists(
    transaction: &Transaction<'_>,
    kind: &str,
    name: &str,
) -> Result<bool, StoreError> {
    transaction
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2",
            params![kind, name],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(StoreError::from)
}

#[cfg(test)]
mod tests {
    use super::normalize_sql;

    #[test]
    fn normalization_ignores_formatting_but_preserves_literal_semantics() {
        assert_eq!(
            normalize_sql("CREATE TABLE demo (value TEXT CHECK(value = 'A B'))"),
            normalize_sql("create table demo(value text check(value='A B'))")
        );
        assert_ne!(
            normalize_sql("CHECK(value = 'A B')"),
            normalize_sql("CHECK(value = 'AB')")
        );
        assert_ne!(
            normalize_sql("CHECK(value = 'TOKEN')"),
            normalize_sql("CHECK(value = 'token')")
        );
        assert_ne!(
            normalize_sql("CHECK(value = 'can''t stop')"),
            normalize_sql("CHECK(value = 'can''tstop')")
        );
    }
}
