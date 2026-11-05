use crate::harness::transcript::TranscriptRecord;
use orchester_protokoll::RunId;
use rusqlite::{params, OptionalExtension};

use super::{database::load_snapshot, RunSnapshot, SqliteRunStore, StoreError};

/// Metadata for every durable run, including completed and interrupted runs.
/// Transcripts remain in their existing store and are read through the owned API.
#[derive(Clone)]
pub struct RunHistoryEntry {
    pub snapshot: RunSnapshot,
    pub created_at: String,
    pub first_prompt: Option<String>,
}

impl SqliteRunStore {
    pub fn history_owned(
        &self,
        owner_actor_id: &str,
        project_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RunHistoryEntry>, StoreError> {
        if owner_actor_id.is_empty()
            || project_id.is_empty()
            || !(1..=512).contains(&limit)
            || offset > i64::MAX as usize
        {
            return Err(StoreError::NotFound);
        }
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT run_id, created_at FROM runs WHERE owner_actor_id = ?1 AND project_id = ?2 ORDER BY created_at, run_id LIMIT ?3 OFFSET ?4",
        )?;
        let rows = statement
            .query_map(
                params![owner_actor_id, project_id, limit as i64, offset as i64],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(|(id, created_at)| {
            let wire = connection.query_row("SELECT wire_json FROM transcript_records WHERE run_id=?1 AND kind='user' ORDER BY ordinal LIMIT 1", [&id], |row| row.get::<_, String>(0)).optional()?;
            let first_prompt = match wire {
                Some(wire) => match self.codec().decode(&wire).map_err(|_| StoreError::Corrupt)? {
                    TranscriptRecord::User(text) => Some(text), _ => return Err(StoreError::Corrupt),
                },
                None => None,
            };
            Ok(RunHistoryEntry {
                snapshot: load_snapshot(&connection, &RunId::from(id), Some(owner_actor_id))?,
                created_at,
                first_prompt,
            })
        }).collect()
    }
}
