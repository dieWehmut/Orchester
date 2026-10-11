use super::{
    empty_detail, finish, push_entry,
    transcript::{arguments, project},
    ActivityDetail, ENTRY_LIMIT, SESSION_LIMIT,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{path::Path, time::Duration};

pub(super) fn read_provider(path: Option<&Path>) -> (&'static str, Vec<ActivityDetail>) {
    let Some(path) = path else {
        return ("not_found", Vec::new());
    };
    if !path.exists() {
        return ("not_found", Vec::new());
    }
    match read_database(path) {
        Ok(rows) => ("available", rows),
        Err(_) => ("unavailable", Vec::new()),
    }
}

fn read_database(path: &Path) -> rusqlite::Result<Vec<ActivityDetail>> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_millis(150))?;
    db.pragma_update(None, "query_only", true)?;
    let mut query = db.prepare("SELECT id, title, directory, time_updated FROM session WHERE time_archived IS NULL ORDER BY time_updated DESC, id DESC LIMIT ?1")?;
    let metadata = query
        .query_map([SESSION_LIMIT as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut sessions = Vec::new();
    for (id, title, directory, updated) in metadata {
        let mut row = empty_detail("opencode", &id, updated);
        row.summary.title = super::clip(&title, 120);
        row.summary.project = project(&directory);
        let mut messages = db.prepare("SELECT substr(data, 1, 65537) FROM message WHERE session_id = ?1 ORDER BY time_created DESC, id DESC LIMIT 1")?;
        if let Some(value) = messages.query_map([&id], |r| r.get::<_, String>(0))?.next() {
            if let Ok(value) = serde_json::from_str::<Value>(&value?) {
                row.summary.state = if !value["error"].is_null() {
                    "interrupted"
                } else if matches!(
                    value["finish"].as_str(),
                    Some("stop" | "end_turn" | "length")
                ) {
                    "completed"
                } else {
                    "active"
                };
            }
        }
        let mut query = db.prepare("SELECT p.id, substr(p.data, 1, 524289), substr(m.data, 1, 65537), p.time_updated FROM part p JOIN message m ON p.message_id = m.id WHERE p.session_id = ?1 ORDER BY p.time_created DESC, p.id DESC LIMIT ?2")?;
        let parts = query
            .query_map(rusqlite::params![id, (ENTRY_LIMIT + 1) as i64], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        row.summary.truncated = parts.len() > ENTRY_LIMIT;
        for (part_id, part, message, changed) in parts.into_iter().rev() {
            row.summary.updated_at = row.summary.updated_at.max(changed);
            // Skip oversized individual records before parsing them into a tree.
            if part.len() > 512 * 1024 || message.len() > 64 * 1024 {
                row.summary.truncated = true;
                continue;
            }
            let (Ok(part), Ok(message)) = (
                serde_json::from_str::<Value>(&part),
                serde_json::from_str::<Value>(&message),
            ) else {
                continue;
            };
            match part["type"].as_str() {
                Some("text") if part["synthetic"].as_bool() != Some(true) => {
                    let kind = if message["role"] == "user" {
                        "user"
                    } else {
                        "assistant"
                    };
                    push_entry(
                        &mut row,
                        &part_id,
                        kind,
                        part["text"].as_str().unwrap_or_default(),
                        None,
                    );
                }
                Some("tool") => {
                    push_entry(
                        &mut row,
                        &part_id,
                        "tool",
                        &arguments(&part["state"]["input"]),
                        part["tool"].as_str(),
                    );
                    let result = part["state"]["output"]
                        .as_str()
                        .or_else(|| part["state"]["error"].as_str())
                        .unwrap_or_default();
                    push_entry(
                        &mut row,
                        &format!("{part_id}-result"),
                        "result",
                        result,
                        part["tool"].as_str(),
                    );
                }
                _ => (),
            }
        }
        finish(&mut row);
        sessions.push(row);
    }
    Ok(sessions)
}
