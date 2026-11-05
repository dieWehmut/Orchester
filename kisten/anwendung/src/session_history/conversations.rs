//! Conversation membership is small application metadata. Model/tool records
//! stay in the durable runtime database; opening history never copies them.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    time::{SystemTime, UNIX_EPOCH},
};

use orchester_laufzeit::harness::{
    run_store::{open_private_database, RunStatus, RunStore, SqliteRunStore},
    service::WorkspaceIdentitySnapshot,
    transcript::TranscriptRecord,
};
use orchester_protokoll::{
    CallId, RunId, StopReason, UiEventEnvelope, UiEventKind, UiToolState, Usage, UI_SCHEMA_VERSION,
};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use super::{
    compact_title, Outcome, SessionHistory, SessionHistoryDetail, SessionHistoryError,
    SessionHistorySummary,
};

const OWNER: &str = "local-user";

struct NewTurn<'a> {
    run_id: &'a RunId,
    conversation_id: Option<&'a str>,
    prompt: &'a str,
    model: Option<&'a str>,
    context_records: usize,
    initial: Option<&'a UiEventEnvelope>,
}

struct Turn {
    public_id: String,
    conversation_id: String,
    durable_id: Option<String>,
    created: u64,
    prompt: Option<String>,
    model: Option<String>,
    status: Option<String>,
    error: Option<String>,
    context_records: usize,
    records: Vec<TranscriptRecord>,
    usage: Usage,
    failed_calls: HashSet<CallId>,
    ui_events: Vec<UiEventEnvelope>,
}

fn unavailable(error: impl std::error::Error + Send + Sync + 'static) -> SessionHistoryError {
    SessionHistoryError::Io(io::Error::other(error))
}

fn identifier(value: &str) -> String {
    let digest = Sha256::digest(format!("orchester-conversation-v1\0{value}"));
    format!(
        "s-{}",
        digest[..16]
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    )
}

impl SessionHistory {
    fn conversation_connection(
        &self,
    ) -> Result<(Connection, WorkspaceIdentitySnapshot), SessionHistoryError> {
        let paths = self.paths.as_ref().ok_or(SessionHistoryError::NotFound)?;
        let identity = WorkspaceIdentitySnapshot::for_workspace(paths.workspace(), OWNER)
            .map_err(unavailable)?;
        let connection = open_private_database(
            &paths
                .run_database()
                .with_file_name("conversation-history.db"),
        )
        .map_err(unavailable)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(unavailable)?;
        connection.execute_batch("PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS conversation_turns (
                public_id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL,
                project_id TEXT NOT NULL, owner TEXT NOT NULL,
                durable_id TEXT, created INTEGER NOT NULL, prompt TEXT NOT NULL,
                model TEXT, status TEXT, error TEXT, context_records INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS conversation_scope ON conversation_turns(owner, project_id, conversation_id);
            CREATE TABLE IF NOT EXISTS conversation_ui_events (
                public_id TEXT NOT NULL, sequence INTEGER NOT NULL, envelope TEXT NOT NULL,
                PRIMARY KEY(public_id, sequence)
            );")
            .map_err(unavailable)?;
        Ok((connection, identity))
    }

    /// Reserve a runtime identity without exposing it as a public history handle.
    pub fn planned_runtime_run_id(public_id: &RunId) -> RunId {
        RunId::from(format!(
            "run-{}",
            identifier(&format!("runtime:{}", public_id.0)).trim_start_matches("s-")
        ))
    }

    /// Commit the user turn before launching the model, including startup failures.
    pub fn begin_conversation_turn(
        &self,
        run_id: &RunId,
        conversation_id: Option<&str>,
        prompt: &str,
        model: Option<&str>,
        context_records: usize,
    ) -> Result<String, SessionHistoryError> {
        self.create_conversation_turn(NewTurn {
            run_id,
            conversation_id,
            prompt,
            model,
            context_records,
            initial: None,
        })
    }

    pub fn begin_conversation_with_event(
        &self,
        initial: &UiEventEnvelope,
        conversation_id: Option<&str>,
        context_records: usize,
    ) -> Result<String, SessionHistoryError> {
        let UiEventKind::UserMessage { text } = &initial.kind else {
            return Err(SessionHistoryError::NotFound);
        };
        if initial.sequence != 1 {
            return Err(SessionHistoryError::NotFound);
        }
        self.create_conversation_turn(NewTurn {
            run_id: &initial.run_id,
            conversation_id,
            prompt: text,
            model: None,
            context_records,
            initial: Some(initial),
        })
    }

    fn create_conversation_turn(&self, turn: NewTurn<'_>) -> Result<String, SessionHistoryError> {
        let NewTurn {
            run_id,
            conversation_id,
            prompt,
            model,
            context_records,
            initial,
        } = turn;
        if let Some(id) = conversation_id {
            if self.detail(id)?.conversation_id.is_none() {
                return Err(SessionHistoryError::NotFound);
            }
        }
        let id = conversation_id
            .map(str::to_owned)
            .unwrap_or_else(|| identifier(&run_id.0));
        let (mut connection, identity) = self.conversation_connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        transaction.execute("INSERT INTO conversation_turns(public_id, conversation_id, project_id, owner, created, prompt, model, context_records, durable_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![run_id.0, id, identity.project_id, OWNER, created, prompt, model, context_records as i64, Self::planned_runtime_run_id(run_id).0]).map_err(unavailable)?;
        if let Some(initial) = initial {
            let envelope = serde_json::to_string(initial).map_err(unavailable)?;
            transaction.execute("INSERT INTO conversation_ui_events(public_id, sequence, envelope) VALUES(?1, ?2, ?3)", params![run_id.0, initial.sequence, envelope]).map_err(unavailable)?;
        }
        transaction.commit().map_err(unavailable)?;
        Ok(id)
    }

    pub fn conversation_for_run(&self, run_id: &RunId) -> Result<String, SessionHistoryError> {
        let (connection, identity) = self.conversation_connection()?;
        connection.query_row("SELECT conversation_id FROM conversation_turns WHERE public_id=?1 AND owner=?2 AND project_id=?3", params![run_id.0, OWNER, identity.project_id], |row| row.get(0)).map_err(|_| SessionHistoryError::NotFound)
    }

    /// Save the already-sanitized public envelope, preserving exact event IDs
    /// and stream sequences across reloads. This contains no runtime evidence.
    pub fn record_conversation_envelope(
        &self,
        event: &UiEventEnvelope,
    ) -> Result<(), SessionHistoryError> {
        let envelope = serde_json::to_string(event).map_err(unavailable)?;
        let (mut connection, identity) = self.conversation_connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        persist_terminal_metadata(&transaction, &identity, &event.run_id, &event.kind)?;
        transaction.execute("INSERT OR IGNORE INTO conversation_ui_events(public_id, sequence, envelope) SELECT public_id, ?1, ?2 FROM conversation_turns WHERE public_id=?3 AND owner=?4 AND project_id=?5", params![event.sequence, envelope, event.run_id.0, OWNER, identity.project_id]).map_err(unavailable)?;
        transaction.commit().map_err(unavailable)?;
        Ok(())
    }

    pub fn conversation_run_events(
        &self,
        run_id: &RunId,
    ) -> Result<Vec<UiEventEnvelope>, SessionHistoryError> {
        let (connection, identity) = self.conversation_connection()?;
        let mut statement = connection.prepare("SELECT e.envelope FROM conversation_ui_events e JOIN conversation_turns t ON t.public_id=e.public_id WHERE t.public_id=?1 AND t.owner=?2 AND t.project_id=?3 ORDER BY e.sequence").map_err(unavailable)?;
        let wire = statement
            .query_map(params![run_id.0, OWNER, identity.project_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(unavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        wire.into_iter()
            .map(|wire| serde_json::from_str(&wire).map_err(unavailable))
            .collect()
    }

    /// Link only an existing run owned by this operator in this workspace.
    pub fn bind_conversation_run(
        &self,
        public_id: &RunId,
        durable_id: &str,
    ) -> Result<(), SessionHistoryError> {
        let paths = self.paths.as_ref().ok_or(SessionHistoryError::NotFound)?;
        let store = SqliteRunStore::open(paths.run_database()).map_err(unavailable)?;
        let (connection, identity) = self.conversation_connection()?;
        let snapshot = store
            .load_run_owned(&RunId::from(durable_id), OWNER)
            .map_err(unavailable)?;
        if snapshot.project_id != identity.project_id {
            return Err(SessionHistoryError::NotFound);
        }
        connection.execute("UPDATE conversation_turns SET durable_id=?1 WHERE public_id=?2 AND owner=?3 AND project_id=?4", params![durable_id, public_id.0, OWNER, identity.project_id]).map_err(unavailable)?;
        Ok(())
    }

    pub fn record_conversation_event(
        &self,
        run_id: &RunId,
        kind: &UiEventKind,
    ) -> Result<(), SessionHistoryError> {
        if !matches!(
            kind,
            UiEventKind::RunStopped { .. } | UiEventKind::Error { .. }
        ) {
            return Ok(());
        }
        let (connection, identity) = self.conversation_connection()?;
        persist_terminal_metadata(&connection, &identity, run_id, kind)
    }

    /// Native user/assistant/tool roles become context for the next turn. System
    /// instructions are rebuilt by the runtime and cannot be supplied by history.
    pub fn conversation_context(
        &self,
        id: &str,
    ) -> Result<Vec<TranscriptRecord>, SessionHistoryError> {
        let turns = self.load_turns(Some(id))?;
        let mut records = Vec::new();
        let mut found = false;
        for turn in turns.into_iter().filter(|turn| turn.conversation_id == id) {
            found = true;
            let mut turn_records = visible_records(&turn);
            let completed = turn_records
                .iter()
                .filter_map(|record| match record {
                    TranscriptRecord::ToolResult { call_id, .. }
                    | TranscriptRecord::ToolResultJson { call_id, .. } => Some(call_id.clone()),
                    _ => None,
                })
                .collect::<HashSet<_>>();
            turn_records.retain(|record| match record {
                TranscriptRecord::User(_) | TranscriptRecord::Assistant(_) => true,
                TranscriptRecord::ToolCall { call_id, .. }
                | TranscriptRecord::ToolResult { call_id, .. }
                | TranscriptRecord::ToolResultJson { call_id, .. } => completed.contains(call_id),
                _ => false,
            });
            for mut record in turn_records {
                match &mut record {
                    TranscriptRecord::ToolCall { call_id, .. }
                    | TranscriptRecord::ToolResult { call_id, .. }
                    | TranscriptRecord::ToolResultJson { call_id, .. } => {
                        *call_id = scoped_call_id(&turn.public_id, call_id);
                    }
                    _ => {}
                }
                records.push(record);
            }
        }
        if !found {
            return Err(SessionHistoryError::NotFound);
        }
        // The runtime's current context window retains at most 64 history
        // records. Keep durable request seeds equally bounded across turns.
        let mut start = records.len().saturating_sub(64);
        while matches!(
            records.get(start),
            Some(TranscriptRecord::ToolResult { .. } | TranscriptRecord::ToolResultJson { .. })
        ) {
            start += 1;
        }
        records.drain(..start);
        Ok(records)
    }

    pub(super) fn conversation_summaries(
        &self,
    ) -> Result<Vec<SessionHistorySummary>, SessionHistoryError> {
        let mut grouped: BTreeMap<String, Vec<Turn>> = BTreeMap::new();
        for turn in self.load_turns(None)? {
            grouped
                .entry(turn.conversation_id.clone())
                .or_default()
                .push(turn);
        }
        Ok(grouped
            .into_iter()
            .map(|(id, turns)| project(id, turns).summary)
            .collect())
    }

    pub(super) fn conversation_detail(
        &self,
        id: &str,
    ) -> Result<SessionHistoryDetail, SessionHistoryError> {
        let turns = self
            .load_turns(Some(id))?
            .into_iter()
            .filter(|turn| turn.conversation_id == id)
            .collect::<Vec<_>>();
        if turns.is_empty() {
            return Err(SessionHistoryError::NotFound);
        }
        Ok(project(id.to_owned(), turns))
    }

    fn load_turns(&self, detail_id: Option<&str>) -> Result<Vec<Turn>, SessionHistoryError> {
        let Some(paths) = self.paths.as_ref() else {
            return Ok(Vec::new());
        };
        let metadata = paths
            .run_database()
            .with_file_name("conversation-history.db");
        if !metadata.exists() && !paths.run_database().exists() {
            return Ok(Vec::new());
        }
        let (connection, identity) = self.conversation_connection()?;
        let mut statement = connection.prepare("SELECT public_id, conversation_id, durable_id, created, prompt, model, status, error, context_records FROM conversation_turns WHERE owner=?1 AND project_id=?2 ORDER BY created, rowid").map_err(unavailable)?;
        let mut turns = statement
            .query_map(params![OWNER, identity.project_id], |row| {
                Ok(Turn {
                    public_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    durable_id: row.get(2)?,
                    created: row.get(3)?,
                    prompt: Some(row.get(4)?),
                    model: row.get(5)?,
                    status: row.get(6)?,
                    error: row.get(7)?,
                    context_records: row.get(8)?,
                    records: Vec::new(),
                    usage: Usage::default(),
                    failed_calls: HashSet::new(),
                    ui_events: Vec::new(),
                })
            })
            .map_err(unavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        for turn in &mut turns {
            if detail_id == Some(turn.conversation_id.as_str()) {
                turn.ui_events =
                    self.conversation_run_events(&RunId::from(turn.public_id.clone()))?;
            }
        }
        if paths.run_database().exists() {
            let store = SqliteRunStore::open(paths.run_database()).map_err(unavailable)?;
            let linked = turns
                .iter()
                .enumerate()
                .filter_map(|(index, turn)| turn.durable_id.as_ref().map(|id| (id.clone(), index)))
                .collect::<HashMap<_, _>>();
            let mut offset = 0;
            loop {
                let batch = store
                    .history_owned(OWNER, &identity.project_id, offset, 512)
                    .map_err(unavailable)?;
                let count = batch.len();
                for entry in batch {
                    let run = entry.snapshot;
                    let conversation_id = linked
                        .get(&run.run_id.0)
                        .map(|index| turns[*index].conversation_id.clone())
                        .unwrap_or_else(|| identifier(&run.run_id.0));
                    let include_records = detail_id == Some(conversation_id.as_str());
                    let records = if include_records {
                        store
                            .transcript_owned(&run.run_id, OWNER)
                            .map_err(unavailable)?
                            .into_iter()
                            .map(|record| record.record)
                            .collect::<Vec<_>>()
                    } else {
                        entry
                            .first_prompt
                            .map(TranscriptRecord::user)
                            .into_iter()
                            .collect()
                    };
                    let failed_calls = if include_records {
                        store
                            .events_owned(&run.run_id, OWNER)
                            .map_err(unavailable)?
                            .into_iter()
                            .filter_map(|event| {
                                matches!(
                                    event.kind,
                                    orchester_protokoll::HarnessEventKind::ToolFailed { .. }
                                )
                                .then_some(event.call_id)
                                .flatten()
                            })
                            .collect()
                    } else {
                        HashSet::new()
                    };
                    let outcome = match run.status {
                        RunStatus::Succeeded => Some("success"),
                        RunStatus::Cancelled => Some("cancelled"),
                        RunStatus::Failed
                        | RunStatus::BudgetExceeded
                        | RunStatus::RepeatedFailure => Some("failed"),
                        _ => None,
                    };
                    let usage = Usage {
                        input_tokens: run.input_tokens_used,
                        output_tokens: run.output_tokens_used,
                        ..Usage::default()
                    };
                    if let Some(index) = linked.get(&run.run_id.0) {
                        turns[*index].records = records;
                        turns[*index].failed_calls = failed_calls;
                        if !include_records {
                            turns[*index].context_records = 0;
                        }
                        turns[*index].usage = usage;
                        if turns[*index].status.is_none() {
                            turns[*index].status = outcome.map(str::to_owned);
                        }
                    } else if records
                        .iter()
                        .any(|record| matches!(record, TranscriptRecord::User(_)))
                    {
                        let public_id = identifier(&run.run_id.0);
                        turns.push(Turn {
                            conversation_id: public_id.clone(),
                            public_id,
                            durable_id: Some(run.run_id.0),
                            created: parse_timestamp(&entry.created_at),
                            prompt: None,
                            model: None,
                            status: outcome.map(str::to_owned),
                            error: None,
                            context_records: 0,
                            records,
                            usage,
                            failed_calls,
                            ui_events: Vec::new(),
                        });
                    }
                }
                if count < 512 {
                    break;
                }
                offset += count;
            }
        }
        turns.sort_by_key(|turn| turn.created);
        Ok(turns)
    }
}

fn visible_records(turn: &Turn) -> Vec<TranscriptRecord> {
    let mut records = turn
        .records
        .iter()
        .skip(turn.context_records)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(prompt) = &turn.prompt {
        if let Some(TranscriptRecord::User(text)) = records.first_mut() {
            *text = prompt.clone();
        } else {
            records.insert(0, TranscriptRecord::user(prompt));
        }
    }
    records
}

fn project(id: String, turns: Vec<Turn>) -> SessionHistoryDetail {
    let mut events = Vec::new();
    let mut prompt = String::new();
    let mut final_text = String::new();
    let mut usage = Usage::default();
    let mut run_ids = Vec::new();
    let mut outcome = Outcome::Success;
    let mut model = None;
    let mut recorded_at_unix = 0;
    for turn in turns {
        let turn_start = events.len();
        let run_id = RunId::from(turn.public_id.clone());
        run_ids.push(run_id.clone());
        recorded_at_unix = recorded_at_unix.max(turn.created);
        model = turn.model.clone().or(model);
        let mut names = HashMap::new();
        for record in visible_records(&turn) {
            let kind = match record {
                TranscriptRecord::User(text) => {
                    if prompt.is_empty() {
                        prompt = text.clone();
                    }
                    UiEventKind::UserMessage { text }
                }
                TranscriptRecord::Assistant(text) => {
                    final_text = text.clone();
                    UiEventKind::Message { text }
                }
                TranscriptRecord::ToolCall { call_id, name, .. } => {
                    names.insert(call_id.clone(), name.clone());
                    UiEventKind::ToolCall {
                        call_id: scoped_call_id(&turn.public_id, &call_id),
                        name,
                        state: UiToolState::Running,
                        detail: None,
                    }
                }
                TranscriptRecord::ToolResult { call_id, output } => {
                    tool_result_kind(&turn, &names, call_id, Some(output))
                }
                TranscriptRecord::ToolResultJson { call_id, payload } => {
                    // Public tool rows expose the already-sanitized observation summary,
                    // not internal evidence, file contents or provider payloads.
                    let detail = payload
                        .get("summary")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned);
                    tool_result_kind(&turn, &names, call_id, detail)
                }
                _ => continue,
            };
            push_event(&mut events, &run_id, turn.created, kind);
        }
        if let Some(message) = turn.error {
            push_event(
                &mut events,
                &run_id,
                turn.created,
                UiEventKind::Error {
                    code: "runtime".into(),
                    message,
                },
            );
        }
        if let Some(status) = turn.status {
            let reason = match status.as_str() {
                "success" => StopReason::Succeeded,
                "cancelled" => StopReason::Cancelled,
                _ => StopReason::Failed,
            };
            outcome = match reason {
                StopReason::Succeeded => Outcome::Success,
                StopReason::Cancelled => Outcome::Cancelled,
                _ => Outcome::Failed,
            };
            push_event(
                &mut events,
                &run_id,
                turn.created,
                UiEventKind::RunStopped { reason },
            );
        }
        if turn
            .ui_events
            .iter()
            .any(|event| matches!(event.kind, UiEventKind::RunStopped { .. }))
        {
            events.truncate(turn_start);
            for mut event in turn.ui_events {
                if let UiEventKind::Message { text } = &event.kind {
                    final_text = text.clone();
                }
                event.sequence = events.len() as u64 + 1;
                events.push(event);
            }
        }
        usage.input_tokens = usage.input_tokens.saturating_add(turn.usage.input_tokens);
        usage.output_tokens = usage.output_tokens.saturating_add(turn.usage.output_tokens);
    }
    SessionHistoryDetail {
        summary: SessionHistorySummary {
            id: id.clone(),
            source: "self",
            recorded_at_unix,
            title: compact_title(&prompt),
            agent: "Orchester".into(),
            model,
            outcome,
            resumable: true,
        },
        conversation_id: Some(id),
        run_ids,
        events,
        prompt,
        final_text,
        usage,
    }
}

fn push_event(events: &mut Vec<UiEventEnvelope>, run_id: &RunId, created: u64, kind: UiEventKind) {
    let sequence = events.len() as u64 + 1;
    let call_id = match &kind {
        UiEventKind::ToolCall { call_id, .. } => Some(call_id.clone()),
        _ => None,
    };
    events.push(UiEventEnvelope {
        schema_version: UI_SCHEMA_VERSION,
        event_id: format!("history-{}-{sequence}", run_id.0).into(),
        run_id: run_id.clone(),
        turn_id: None,
        call_id,
        sequence,
        occurred_at: time::OffsetDateTime::from_unix_timestamp(created as i64)
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        kind,
    });
}

fn tool_result_kind(
    turn: &Turn,
    names: &HashMap<CallId, String>,
    call_id: CallId,
    detail: Option<String>,
) -> UiEventKind {
    let name = names
        .get(&call_id)
        .cloned()
        .unwrap_or_else(|| "tool".into());
    let state = if turn.failed_calls.contains(&call_id) {
        UiToolState::Failed
    } else {
        UiToolState::Succeeded
    };
    UiEventKind::ToolCall {
        call_id: scoped_call_id(&turn.public_id, &call_id),
        name,
        state,
        detail: detail.map(|text| super::bounded_text(&text, 16_384)),
    }
}

fn scoped_call_id(public_id: &str, call_id: &CallId) -> CallId {
    CallId::from(format!(
        "call-{}",
        identifier(&format!("{public_id}\0{}", call_id.0)).trim_start_matches("s-")
    ))
}

fn parse_timestamp(value: &str) -> u64 {
    value
        .trim_start_matches("unix:")
        .parse()
        .unwrap_or_else(|_| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                .map(|time| time.unix_timestamp().max(0) as u64)
                .unwrap_or_default()
        })
}

fn persist_terminal_metadata(
    connection: &Connection,
    identity: &WorkspaceIdentitySnapshot,
    run_id: &RunId,
    kind: &UiEventKind,
) -> Result<(), SessionHistoryError> {
    match kind {
        UiEventKind::RunStopped { reason } => {
            let status = match reason {
                StopReason::Succeeded => "success",
                StopReason::Cancelled => "cancelled",
                _ => "failed",
            };
            connection.execute("UPDATE conversation_turns SET status=?1 WHERE public_id=?2 AND owner=?3 AND project_id=?4", params![status, run_id.0, OWNER, identity.project_id]).map_err(unavailable)?;
        }
        UiEventKind::Error { .. } => {
            connection.execute("UPDATE conversation_turns SET error='The run could not be completed.' WHERE public_id=?1 AND owner=?2 AND project_id=?3", params![run_id.0, OWNER, identity.project_id]).map_err(unavailable)?;
        }
        _ => {}
    }
    Ok(())
}
