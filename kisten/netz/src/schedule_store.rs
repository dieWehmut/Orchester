//! Durable per-workspace schedules with transactional, cross-process claims.
use crate::{
    api_error::ApiErrorCode, model_selection::ModelSelection, run_contract::StartRunRequest,
};
use orchester_anwendung::OrchesterPaths;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

pub(crate) type Result<T> = std::result::Result<T, ApiErrorCode>;
const LEASE_SECONDS: i64 = 60;

pub(crate) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
pub(crate) fn identifier() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ApiErrorCode::Internal)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScheduleInput {
    pub name: String,
    pub prompt: String,
    pub enabled: bool,
    pub first_run_at: i64,
    pub interval_seconds: Option<i64>,
}

impl ScheduleInput {
    pub fn validate(&self) -> Result<()> {
        let name = self.name.trim();
        if name.is_empty()
            || name.chars().count() > 80
            || name.chars().any(char::is_control)
            || self.prompt.chars().count() > 8000
            || self.first_run_at <= 0
            || self.first_run_at > 253402300799
            || self
                .interval_seconds
                .is_some_and(|v| !(60..=31_536_000).contains(&v))
        {
            return Err(ApiErrorCode::ValidationFailed);
        }
        StartRunRequest {
            prompt: self.prompt.clone(),
            resume: None,
        }
        .validate()
        .map_err(|_| ApiErrorCode::ValidationFailed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ScheduleRun {
    pub id: String,
    pub run_id: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Schedule {
    pub id: String,
    #[serde(flatten)]
    pub input: ScheduleInput,
    pub next_run_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub history: Vec<ScheduleRun>,
}

pub(crate) struct Claim {
    pub task: Schedule,
    pub execution: ScheduleRun,
    pub selection: ModelSelection,
}

pub(crate) struct ScheduleStore {
    db: Connection,
    workspace: String,
}

impl ScheduleStore {
    pub fn open(paths: &OrchesterPaths) -> Result<Self> {
        std::fs::create_dir_all(paths.state_root()).map_err(|_| ApiErrorCode::Unavailable)?;
        Self::at(
            &paths.state_root().join("schedules.db"),
            &paths.workspace().to_string_lossy(),
        )
    }

    fn at(path: &Path, workspace: &str) -> Result<Self> {
        let db = Connection::open(path).map_err(|_| ApiErrorCode::Unavailable)?;
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|_| ApiErrorCode::Unavailable)?;
        db.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS schedules (
              id TEXT PRIMARY KEY, workspace TEXT NOT NULL, input TEXT NOT NULL,
              selection TEXT NOT NULL, enabled INTEGER NOT NULL, next_run_at INTEGER NOT NULL,
              created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS schedules_due ON schedules(workspace, enabled, next_run_at);
            CREATE TABLE IF NOT EXISTS schedule_runs (
              id TEXT PRIMARY KEY, schedule_id TEXT NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
              owner TEXT NOT NULL, lease_until INTEGER NOT NULL, run_id TEXT,
              started_at INTEGER NOT NULL, finished_at INTEGER, status TEXT NOT NULL, error_code TEXT);
            CREATE INDEX IF NOT EXISTS schedule_history ON schedule_runs(schedule_id, started_at DESC);")
            .map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(Self {
            db,
            workspace: workspace.into(),
        })
    }

    fn recover(&self, timestamp: i64) -> Result<()> {
        self.db.execute("UPDATE schedule_runs SET status='interrupted',finished_at=?1,error_code='interrupted'
            WHERE finished_at IS NULL AND lease_until<=?1", [timestamp]).map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Schedule>> {
        self.recover(now())?;
        let mut statement = self.db.prepare("SELECT id,input,enabled,next_run_at,created_at,updated_at FROM schedules WHERE workspace=?1 ORDER BY created_at DESC,id")
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let rows = statement
            .query_map([&self.workspace], read_schedule)
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let mut schedules = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| ApiErrorCode::Unavailable)?;
        for task in &mut schedules {
            task.history = self.history(&task.id)?;
        }
        Ok(schedules)
    }

    pub fn get(&self, id: &str) -> Result<Schedule> {
        let mut task = self.db.query_row("SELECT id,input,enabled,next_run_at,created_at,updated_at FROM schedules WHERE id=?1 AND workspace=?2",
            params![id, self.workspace], read_schedule).optional().map_err(|_| ApiErrorCode::Unavailable)?.ok_or(ApiErrorCode::NotFound)?;
        task.history = self.history(id)?;
        Ok(task)
    }

    fn history(&self, id: &str) -> Result<Vec<ScheduleRun>> {
        let mut statement = self.db.prepare("SELECT id,run_id,started_at,finished_at,status,error_code FROM schedule_runs WHERE schedule_id=?1 ORDER BY started_at DESC,rowid DESC LIMIT 20")
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let runs = statement
            .query_map([id], |row| {
                Ok(ScheduleRun {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    started_at: row.get(2)?,
                    finished_at: row.get(3)?,
                    status: row.get(4)?,
                    error_code: row.get(5)?,
                })
            })
            .map_err(|_| ApiErrorCode::Unavailable)?;
        runs.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| ApiErrorCode::Unavailable)
    }

    pub fn save(
        &mut self,
        id: Option<&str>,
        input: ScheduleInput,
        selection: ModelSelection,
    ) -> Result<Schedule> {
        input.validate()?;
        let timestamp = now();
        self.recover(timestamp)?;
        let key = match id {
            Some(id) => id.to_owned(),
            None => identifier()?,
        };
        let json = serde_json::to_string(&input).map_err(|_| ApiErrorCode::Internal)?;
        let selection = serde_json::to_string(&selection).map_err(|_| ApiErrorCode::Internal)?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ApiErrorCode::Unavailable)?;
        if id.is_some() {
            let old: Option<(String, i64)> = transaction
                .query_row(
                    "SELECT input,next_run_at FROM schedules WHERE id=?1 AND workspace=?2",
                    params![key, self.workspace],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(|_| ApiErrorCode::Unavailable)?;
            let (old_input, old_next) = old.ok_or(ApiErrorCode::NotFound)?;
            if active(&transaction, &key)? {
                return Err(ApiErrorCode::Conflict);
            }
            let old: ScheduleInput =
                serde_json::from_str(&old_input).map_err(|_| ApiErrorCode::Unavailable)?;
            let next = if old.first_run_at == input.first_run_at
                && old.interval_seconds == input.interval_seconds
            {
                old_next
            } else {
                input.first_run_at
            };
            transaction.execute("UPDATE schedules SET input=?1,selection=?2,enabled=?3,next_run_at=?4,updated_at=?5 WHERE id=?6",
                params![json,selection,input.enabled,next,timestamp,key]).map_err(|_| ApiErrorCode::Unavailable)?;
        } else {
            let count: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM schedules WHERE workspace=?1",
                    [&self.workspace],
                    |r| r.get(0),
                )
                .map_err(|_| ApiErrorCode::Unavailable)?;
            if count >= 100 {
                return Err(ApiErrorCode::Conflict);
            }
            transaction
                .execute(
                    "INSERT INTO schedules VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
                    params![
                        key,
                        self.workspace,
                        json,
                        selection,
                        input.enabled,
                        input.first_run_at,
                        timestamp
                    ],
                )
                .map_err(|_| ApiErrorCode::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| ApiErrorCode::Unavailable)?;
        self.get(&key)
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        self.recover(now())?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ApiErrorCode::Unavailable)?;
        if active(&transaction, id)? {
            return Err(ApiErrorCode::Conflict);
        }
        let count = transaction
            .execute(
                "DELETE FROM schedules WHERE id=?1 AND workspace=?2",
                params![id, self.workspace],
            )
            .map_err(|_| ApiErrorCode::Unavailable)?;
        if count == 0 {
            return Err(ApiErrorCode::NotFound);
        }
        transaction.commit().map_err(|_| ApiErrorCode::Unavailable)
    }

    pub fn claim(
        &mut self,
        id: Option<&str>,
        owner: &str,
        timestamp: i64,
    ) -> Result<Option<Claim>> {
        self.recover(timestamp)?;
        let transaction = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let key = if let Some(id) = id {
            id.to_owned()
        } else {
            let due: Option<String> = transaction.query_row("SELECT id FROM schedules WHERE workspace=?1 AND enabled=1 AND next_run_at<=?2
                AND NOT EXISTS(SELECT 1 FROM schedule_runs WHERE schedule_id=schedules.id AND finished_at IS NULL) ORDER BY next_run_at,id LIMIT 1",
                params![self.workspace,timestamp], |r| r.get(0)).optional().map_err(|_| ApiErrorCode::Unavailable)?;
            let Some(key) = due else { return Ok(None) };
            key
        };
        let record = transaction.query_row("SELECT id,input,enabled,next_run_at,created_at,updated_at FROM schedules WHERE id=?1 AND workspace=?2",
            params![key,self.workspace], read_schedule).optional().map_err(|_| ApiErrorCode::Unavailable)?.ok_or(ApiErrorCode::NotFound)?;
        if active(&transaction, &key)? {
            return Err(ApiErrorCode::Conflict);
        }
        let selection: String = transaction
            .query_row("SELECT selection FROM schedules WHERE id=?1", [&key], |r| {
                r.get(0)
            })
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let selection = serde_json::from_str(&selection).map_err(|_| ApiErrorCode::Unavailable)?;
        let execution = ScheduleRun {
            id: identifier()?,
            run_id: None,
            started_at: timestamp,
            finished_at: None,
            status: "running".into(),
            error_code: None,
        };
        transaction.execute("INSERT INTO schedule_runs(id,schedule_id,owner,lease_until,started_at,status) VALUES(?1,?2,?3,?4,?5,'running')",
            params![execution.id,key,owner,timestamp+LEASE_SECONDS,timestamp]).map_err(|_| ApiErrorCode::Unavailable)?;
        // A manual run does not consume or reschedule a future scheduled occurrence.
        if id.is_none() {
            let next = record
                .input
                .interval_seconds
                .map(|period| timestamp.saturating_add(period))
                .unwrap_or(record.next_run_at);
            transaction
                .execute(
                    "UPDATE schedules SET enabled=?1,next_run_at=?2,updated_at=?3 WHERE id=?4",
                    params![
                        record.input.interval_seconds.is_some(),
                        next,
                        timestamp,
                        key
                    ],
                )
                .map_err(|_| ApiErrorCode::Unavailable)?;
        }
        transaction.execute("DELETE FROM schedule_runs WHERE schedule_id=?1 AND finished_at IS NOT NULL AND id NOT IN
            (SELECT id FROM schedule_runs WHERE schedule_id=?1 ORDER BY started_at DESC,rowid DESC LIMIT 20)", [&key]).map_err(|_| ApiErrorCode::Unavailable)?;
        transaction
            .commit()
            .map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(Some(Claim {
            task: record,
            execution,
            selection,
        }))
    }

    pub fn bind(&self, id: &str, owner: &str, run_id: &str) -> Result<()> {
        let count = self.db.execute("UPDATE schedule_runs SET run_id=?1 WHERE id=?2 AND owner=?3 AND finished_at IS NULL", params![run_id,id,owner]).map_err(|_| ApiErrorCode::Unavailable)?;
        if count == 1 {
            Ok(())
        } else {
            Err(ApiErrorCode::Conflict)
        }
    }

    pub fn heartbeat(&self, owner: &str) -> Result<()> {
        self.db
            .execute(
                "UPDATE schedule_runs SET lease_until=?1 WHERE owner=?2 AND finished_at IS NULL",
                params![now() + LEASE_SECONDS, owner],
            )
            .map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(())
    }

    pub fn finish(&self, id: &str, owner: &str, status: &str, error: Option<&str>) -> Result<()> {
        self.db.execute("UPDATE schedule_runs SET status=?1,error_code=?2,finished_at=?3 WHERE id=?4 AND owner=?5 AND finished_at IS NULL",
            params![status,error,now(),id,owner]).map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(())
    }
}

fn active(db: &Connection, id: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM schedule_runs WHERE schedule_id=?1 AND finished_at IS NULL)",
        [id],
        |row| row.get(0),
    )
    .map_err(|_| ApiErrorCode::Unavailable)
}

fn read_schedule(row: &rusqlite::Row<'_>) -> rusqlite::Result<Schedule> {
    let json: String = row.get(1)?;
    let mut input: ScheduleInput = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    input.enabled = row.get(2)?;
    Ok(Schedule {
        id: row.get(0)?,
        input,
        next_run_at: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        history: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> ScheduleInput {
        ScheduleInput {
            name: "Daily check".into(),
            prompt: "Inspect changes".into(),
            enabled: true,
            first_run_at: now() - 100,
            interval_seconds: Some(60),
        }
    }

    #[test]
    fn claims_persist_prevent_overlap_recover_and_isolate_workspaces() {
        let root = std::env::temp_dir().join(format!("schedule-store-{}", identifier().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("test.db");
        let mut a = ScheduleStore::at(&db, "workspace-a").unwrap();
        let task = a.save(None, input(), ModelSelection::default()).unwrap();
        let timestamp = now();
        let claim = a.claim(None, "one", timestamp).unwrap().unwrap();
        let mut b = ScheduleStore::at(&db, "workspace-a").unwrap();
        assert!(b.claim(None, "two", timestamp).unwrap().is_none());
        assert!(matches!(
            b.claim(Some(&task.id), "two", timestamp),
            Err(ApiErrorCode::Conflict)
        ));
        assert!(matches!(b.delete(&task.id), Err(ApiErrorCode::Conflict)));
        assert!(matches!(
            b.save(Some(&task.id), input(), ModelSelection::default()),
            Err(ApiErrorCode::Conflict)
        ));
        let other = ScheduleStore::at(&db, "workspace-b").unwrap();
        assert!(other.list().unwrap().is_empty());
        assert!(matches!(other.get(&task.id), Err(ApiErrorCode::NotFound)));
        assert_eq!(b.get(&task.id).unwrap().next_run_at, timestamp + 60);
        b.recover(timestamp + 61).unwrap();
        assert_eq!(b.get(&task.id).unwrap().history[0].status, "interrupted");
        assert!(b.claim(None, "two", timestamp + 61).unwrap().is_some());
        // The previous owner cannot turn an interrupted run into a success.
        a.finish(&claim.execution.id, "one", "succeeded", None)
            .unwrap();
        assert_eq!(a.get(&task.id).unwrap().history[1].status, "interrupted");
        drop((a, b, other));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_shot_disables_and_manual_run_keeps_future_due_time() {
        let mut store = ScheduleStore::at(Path::new(":memory:"), "project").unwrap();
        let mut data = input();
        data.interval_seconds = None;
        let task = store.save(None, data, ModelSelection::default()).unwrap();
        let manual = store
            .claim(Some(&task.id), "owner", now())
            .unwrap()
            .unwrap();
        assert!(store.get(&task.id).unwrap().input.enabled);
        store
            .finish(&manual.execution.id, "owner", "failed", Some("runtime"))
            .unwrap();
        let scheduled = store.claim(None, "owner", now()).unwrap().unwrap();
        assert!(!store.get(&task.id).unwrap().input.enabled);
        store
            .finish(&scheduled.execution.id, "owner", "succeeded", None)
            .unwrap();
        assert!(store.claim(None, "owner", now() + 3600).unwrap().is_none());
        store.delete(&task.id).unwrap();
        assert!(store.list().unwrap().is_empty());
    }
}
