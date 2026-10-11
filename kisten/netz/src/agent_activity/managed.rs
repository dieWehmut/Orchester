use super::{empty_detail, finish, push_entry, ActivityDetail, SESSION_LIMIT};
use crate::{RunStateDto, ServerContext};
use orchester_protokoll::UiEventKind;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) async fn snapshot(context: &ServerContext) -> Vec<ActivityDetail> {
    let Ok(handles) = context.runs().all() else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for handle in handles {
        let Ok(snapshot) = handle.snapshot().await else {
            continue;
        };
        let updated = OffsetDateTime::parse(&snapshot.updated_at, &Rfc3339)
            .map(|time| (time.unix_timestamp_nanos() / 1_000_000) as i64)
            .unwrap_or_default();
        let mut row = empty_detail("orchester", &snapshot.run_id.0, updated);
        row.summary.source = "managed";
        row.summary.state = match snapshot.state {
            RunStateDto::Succeeded => "completed",
            RunStateDto::Failed | RunStateDto::Cancelled => "interrupted",
            _ => "active",
        };
        row.summary.truncated = snapshot.oldest_sequence > 1;
        let mut delta = String::new();
        let mut delta_id = String::new();
        for event in snapshot.events {
            let id = event.sequence.to_string();
            match event.kind {
                UiEventKind::UserMessage { text } => {
                    row.summary.title = super::clip(&text, 120);
                    push_entry(&mut row, &id, "user", &text, None);
                }
                UiEventKind::Message { text } => {
                    push_entry(&mut row, &id, "assistant", &text, None)
                }
                UiEventKind::MessageDelta { text, final_chunk } => {
                    if delta_id.is_empty() {
                        delta_id = id;
                    }
                    delta.push_str(&text);
                    push_entry(&mut row, &delta_id, "assistant", &delta, None);
                    if final_chunk {
                        delta.clear();
                        delta_id.clear();
                    }
                }
                UiEventKind::ToolCall { name, detail, .. } => push_entry(
                    &mut row,
                    &id,
                    "tool",
                    detail.as_deref().unwrap_or_default(),
                    Some(&name),
                ),
                UiEventKind::Error { message, .. } => {
                    push_entry(&mut row, &id, "status", &message, None)
                }
                _ => (),
            }
        }
        finish(&mut row);
        rows.push(row);
    }
    rows.sort_by(|a, b| b.summary.updated_at.cmp(&a.summary.updated_at));
    rows.truncate(SESSION_LIMIT);
    rows
}
