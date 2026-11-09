//! Read-only task projections for the terminal command center.
//! Native provider session identifiers are never treated as managed resume handles.
use std::{
    collections::HashSet,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use orchester_anwendung::{OrchesterPaths, SelfAgentHost};
use orchester_laufzeit::{
    harness::service::{SelfAgentResumeAvailability, SelfAgentResumeEntry},
    SessionRecord, SessionStore,
};
use orchester_netz::{ActivityDetail, AgentActivityRoots, LocalAgentActivity};
use orchester_protokoll::Outcome;
use sha2::{Digest, Sha256};

const TEXT_LIMIT: usize = 48 * 1024;
const HISTORY_LIMIT: usize = 100;
const UNKNOWN: &str = "—";
static ACTIVITY: OnceLock<LocalAgentActivity> = OnceLock::new();

fn local_activity() -> &'static LocalAgentActivity {
    ACTIVITY.get_or_init(|| LocalAgentActivity::new(AgentActivityRoots::from_environment()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Working,
    NeedsYou,
    Ready,
    Inactive,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    /// Source identity, kept separate from the possibly abbreviated display label.
    pub project_key: String,
    pub project: String,
    pub title: String,
    pub status: TaskStatus,
    pub updated: String,
    pub model: String,
    pub reasoning: String,
    pub prompt: String,
    pub details: String,
    pub resume_handle: Option<String>,
}

#[derive(Debug)]
pub struct LoadResult {
    pub tasks: Vec<Task>,
    pub warnings: Vec<String>,
}

pub fn load() -> Result<LoadResult, String> {
    let paths =
        OrchesterPaths::discover().map_err(|_| "Workspace could not be read.".to_owned())?;
    load_from(&paths, local_activity())
}

fn load_from(paths: &OrchesterPaths, activity: &LocalAgentActivity) -> Result<LoadResult, String> {
    let now = now_ms();
    let mut tasks = Vec::new();
    let mut errors = Vec::new();
    match SessionStore::new(paths.session_log()).load() {
        Ok(records) => {
            // Continued native sessions have one row, displaying their latest recorded turn.
            let mut seen = HashSet::new();
            for (index, record) in records.iter().enumerate().rev() {
                if let Some(id) = record.session_id.as_ref() {
                    if !seen.insert((&record.agent, id)) {
                        continue;
                    }
                }
                tasks.push((recorded_ms(record), history_task(index, record, now)));
                if tasks.len() == HISTORY_LIMIT {
                    break;
                }
            }
        }
        Err(_) => errors.push("Recorded adapter history could not be read.".to_owned()),
    }
    match activity.snapshot() {
        Ok(snapshot) => {
            for source in snapshot.sources {
                match source.status {
                    "unavailable" => {
                        errors.push(format!("{} activity could not be read.", source.provider))
                    }
                    "partial" => errors.push(format!(
                        "Some {} activity could not be read.",
                        source.provider
                    )),
                    _ => (),
                }
            }
            tasks.extend(
                snapshot
                    .sessions
                    .iter()
                    .map(|row| (row.summary.updated_at, external_task(row, now))),
            );
        }
        Err(_) => errors.push("Local provider activity could not be read.".to_owned()),
    }
    // No config, database, credentials or runtime need to be created for an empty catalog.
    if paths.run_database().exists() {
        match SelfAgentHost::for_paths(paths).resume_catalog() {
            Ok(catalog) => {
                let project = paths.workspace().to_string_lossy();
                tasks.extend(
                    catalog
                        .entries
                        .iter()
                        .map(|entry| (0, resume_task(entry, &project))),
                );
            }
            Err(_) => errors.push("Managed resume history could not be read.".to_owned()),
        }
    }
    // An unavailable source must not conceal readable tasks from another source.
    if tasks.is_empty() && !errors.is_empty() {
        return Err(errors.join(" "));
    }
    // Keep the resume catalog's known newest-first ordering when timestamps are absent.
    tasks.sort_by(|(left_at, _), (right_at, _)| right_at.cmp(left_at));
    Ok(LoadResult {
        tasks: tasks.into_iter().map(|(_, task)| task).collect(),
        warnings: errors,
    })
}

pub fn detail(task: &Task) -> Result<String, String> {
    if task.id.starts_with("external:") {
        return detail_from(task, local_activity());
    }
    Ok(task.details.clone())
}

fn detail_from(task: &Task, activity: &LocalAgentActivity) -> Result<String, String> {
    let Some(id) = task.id.strip_prefix("external:") else {
        return Ok(task.details.clone());
    };
    let row = activity
        .detail(id)
        .map_err(|_| "Local provider conversation could not be read.".to_owned())?
        .ok_or_else(|| "Local provider conversation is no longer available.".to_owned())?;
    Ok(external_task(&row, now_ms()).details)
}

fn history_task(index: usize, record: &SessionRecord, now: i64) -> Task {
    let prompt = safe_text(&record.prompt, TEXT_LIMIT);
    let project = project_label(&record.cwd.to_string_lossy());
    let outcome = match record.outcome {
        Outcome::Success => "completed",
        Outcome::Failed => "failed",
        Outcome::Cancelled => "cancelled",
    };
    Task {
        id: record.session_id.as_ref().map(|id| {
            format!("history:{:x}", Sha256::digest(format!("{}\0{id}", record.agent)))
        }).unwrap_or_else(|| format!("history:{index}:{}", record.recorded_at_unix)),
        project_key: format!("local:{}", record.cwd.display()),
        project,
        title: title(&prompt),
        status: if record.outcome == Outcome::Success {
            TaskStatus::Ready
        } else {
            TaskStatus::Inactive
        },
        updated: relative_time(recorded_ms(record), now),
        model: record
            .model
            .as_deref()
            .map(|value| safe_text(value, 120))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| UNKNOWN.into()),
        reasoning: UNKNOWN.into(),
        details: safe_text(
            &format!(
                "{}\n\nAgent: {}\nState: {outcome}\nProject: {}\nModel: {}\n\nRecorded adapter session · read-only\n\nPrompt\n{prompt}\n\nResponse\n{}",
                title(&prompt),
                record.agent,
                record.cwd.display(),
                record.model.as_deref().unwrap_or(UNKNOWN),
                record.final_text,
            ),
            TEXT_LIMIT,
        ),
        prompt,
        resume_handle: None,
    }
}

fn external_task(row: &ActivityDetail, now: i64) -> Task {
    let summary = &row.summary;
    let prompt = row
        .entries
        .iter()
        .find(|entry| entry.kind == "user")
        .map(|entry| safe_text(&entry.text, TEXT_LIMIT))
        .unwrap_or_else(|| UNKNOWN.into());
    let title = if summary.title.trim().is_empty() {
        "Untitled conversation".into()
    } else {
        safe_text(&summary.title, 120)
    };
    let mut details = format!(
        "{title}\n\nProvider: {}\nState: {}\nProject: {}\n\nExternal provider session · read-only\n",
        summary.provider, summary.state, summary.project
    );
    if summary.truncated {
        details.push_str("Showing a bounded excerpt of the provider transcript.\n");
    }
    for entry in &row.entries {
        if details.len() >= TEXT_LIMIT {
            break;
        }
        details.push_str(&format!(
            "\n{}{}\n{}\n",
            entry.kind,
            entry
                .tool
                .as_ref()
                .map(|tool| format!(" · {tool}"))
                .unwrap_or_default(),
            entry.text
        ));
    }
    Task {
        id: format!("external:{}", summary.id),
        project_key: summary
            .project_id
            .as_ref()
            .map(|id| format!("external:{id}"))
            .unwrap_or_else(|| format!("external-session:{}", summary.id)),
        project: if summary.project.trim().is_empty() {
            "Unknown project".into()
        } else {
            safe_text(&summary.project, 256)
        },
        title,
        status: external_status(summary.state),
        updated: relative_time(summary.updated_at, now),
        model: UNKNOWN.into(),
        reasoning: UNKNOWN.into(),
        prompt,
        details: safe_text(&details, TEXT_LIMIT),
        resume_handle: None,
    }
}

fn resume_task(entry: &SelfAgentResumeEntry, project: &str) -> Task {
    let (status, availability) = match entry.availability {
        SelfAgentResumeAvailability::Ready => (TaskStatus::Ready, "ready to resume"),
        SelfAgentResumeAvailability::ApprovalRequired => {
            (TaskStatus::NeedsYou, "approval required")
        }
        SelfAgentResumeAvailability::ReconciliationRequired => {
            (TaskStatus::NeedsYou, "reconciliation required")
        }
        SelfAgentResumeAvailability::Unsupported => {
            (TaskStatus::Inactive, "continuation unsupported")
        }
    };
    let title = format!(
        "Self-agent run {}",
        entry.handle.chars().take(12).collect::<String>()
    );
    Task {
        id: format!("managed:{}", entry.handle),
        project_key: format!("local:{project}"),
        project: project_label(project),
        title: title.clone(),
        status,
        updated: UNKNOWN.into(),
        model: UNKNOWN.into(),
        reasoning: UNKNOWN.into(),
        prompt: UNKNOWN.into(),
        details: safe_text(
            &format!(
                "{title}\n\nState: {availability}\nProject: {project}\n\n{}\n\nThe original prompt, model and update time are unavailable for this saved run.",
                if entry.availability == SelfAgentResumeAvailability::Ready {
                    "Press r in the task list to resume this managed run."
                } else {
                    "This run cannot be resumed from the task list until its continuation requirements are resolved."
                }
            ),
            TEXT_LIMIT,
        ),
        resume_handle: (entry.availability == SelfAgentResumeAvailability::Ready)
            .then(|| entry.handle.clone()),
    }
}

fn external_status(state: &str) -> TaskStatus {
    match state {
        "active" => TaskStatus::Working,
        "completed" => TaskStatus::Ready,
        _ => TaskStatus::Inactive,
    }
}

fn project_label(path: &str) -> String {
    // Keep full stored paths so same-named workspaces remain distinguishable.
    // Relative history cannot safely be resolved against today's directory.
    if path.is_empty() {
        "Unknown project".into()
    } else {
        safe_text(path, 256)
    }
}

fn title(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or_default().trim();
    if line.is_empty() {
        "Untitled conversation".into()
    } else {
        safe_text(line, 120)
    }
}

fn safe_text(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(limit)
        .collect()
}

fn recorded_ms(record: &SessionRecord) -> i64 {
    i64::try_from(record.recorded_at_unix.saturating_mul(1000)).unwrap_or(i64::MAX)
}

fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

fn relative_time(updated: i64, now: i64) -> String {
    if updated <= 0 {
        return UNKNOWN.into();
    }
    let seconds = now.saturating_sub(updated).max(0) / 1000;
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_laufzeit::harness::service::{SelfAgentResumeEntry, SelfAgentResumeStep};
    use orchester_protokoll::{Outcome, Usage};
    use std::path::PathBuf;

    #[test]
    fn partial_provider_failure_keeps_readable_tasks_and_reports_the_missing_source() {
        let root = std::env::temp_dir().join(format!(
            "orchester-command-partial-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("codex")).unwrap();
        std::fs::write(root.join("codex/task.jsonl"),
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Readable task\"}]}}\n"
        ).unwrap();
        std::fs::write(root.join("broken.db"), "invalid sqlite data").unwrap();
        // The parser rejects oversized records while retaining readable sibling transcripts.
        std::fs::File::create(root.join("codex/oversized.jsonl"))
            .unwrap()
            .set_len(16 * 1024 * 1024 + 1)
            .unwrap();
        let paths = OrchesterPaths::new(root.join("home"), root.join("workspace"));
        let activity = LocalAgentActivity::new(AgentActivityRoots {
            codex: Some(root.join("codex")),
            opencode: Some(root.join("broken.db")),
            ..AgentActivityRoots::default()
        });
        let result = load_from(&paths, &activity).unwrap();
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.tasks[0].title, "Readable task");
        assert_eq!(
            result.warnings,
            [
                "Some codex activity could not be read.",
                "opencode activity could not be read.",
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn loads_local_history_and_external_activity_without_merging_resume_capabilities() {
        let root = std::env::temp_dir().join(format!(
            "orchester-command-data-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("codex")).unwrap();
        let paths = OrchesterPaths::new(root.join("home"), root.join("workspace"));
        let store = SessionStore::new(paths.session_log());
        let mut record = SessionRecord {
            recorded_at_unix: 10,
            agent: "claude".into(),
            session_id: Some("native-id".into()),
            prompt: "Old turn".into(),
            cwd: PathBuf::from("relative/project"),
            model: Some("historic-model".into()),
            outcome: Outcome::Success,
            final_text: "Done".into(),
            usage: Usage::default(),
        };
        store.append(&record).unwrap();
        record.recorded_at_unix = 20;
        record.prompt = "Latest turn".into();
        store.append(&record).unwrap();
        std::fs::write(root.join("codex/task.jsonl"), concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/work/external-project\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Live provider task\"}]}}\n"
        )).unwrap();
        let activity = LocalAgentActivity::new(AgentActivityRoots {
            codex: Some(root.join("codex")),
            ..AgentActivityRoots::default()
        });
        let result = load_from(&paths, &activity).unwrap();
        assert!(result.warnings.is_empty());
        let tasks = result.tasks;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Live provider task");
        assert_eq!(tasks[0].project, "external-project");
        assert_eq!(tasks[0].status, TaskStatus::Working);
        assert_eq!(tasks[0].model, "—");
        assert!(tasks[0].resume_handle.is_none());
        assert_eq!(tasks[1].title, "Latest turn");
        assert_eq!(tasks[1].project, "relative/project");
        assert_eq!(tasks[1].model, "historic-model");
        assert!(tasks[1].resume_handle.is_none());
        assert!(!paths.run_database().exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn opens_historical_summary_transcripts_on_demand_without_resume_handles() {
        use std::time::Duration;
        let root = std::env::temp_dir().join(format!(
            "orchester-command-history-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let sessions = root.join("codex");
        std::fs::create_dir_all(&sessions).unwrap();
        let prompt = format!("Historical 000 {}", "preserved user context ".repeat(20));
        for index in 0..140 {
            let path = sessions.join(format!("task-{index:03}.jsonl"));
            let text = if index == 0 {
                prompt.clone()
            } else {
                format!("Historical {index:03}")
            };
            let user = serde_json::json!({"type":"response_item", "payload":{"type":"message", "role":"user", "content":[{"type":"input_text", "text":text}]}});
            let assistant = serde_json::json!({"type":"response_item", "payload":{"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":"Historical assistant body"}]}});
            std::fs::write(&path, format!("{user}\n{assistant}\n")).unwrap();
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(UNIX_EPOCH + Duration::from_secs(1_000 + index))
                .unwrap();
        }
        let paths = OrchesterPaths::new(root.join("home"), root.join("workspace"));
        let activity = LocalAgentActivity::new(AgentActivityRoots {
            codex: Some(sessions.clone()),
            ..AgentActivityRoots::default()
        });
        let result = load_from(&paths, &activity).unwrap();
        assert_eq!(result.tasks.len(), 140);
        let task = result
            .tasks
            .iter()
            .find(|task| task.title.starts_with("Historical 000"))
            .unwrap();
        assert!(!task.details.contains("Historical assistant body"));
        let details = detail_from(task, &activity).unwrap();
        assert!(details.contains(&prompt));
        assert!(details.contains("Historical assistant body"));
        assert!(task.resume_handle.is_none());
        assert!(!paths.run_database().exists());
        std::fs::remove_file(sessions.join("task-000.jsonl")).unwrap();
        assert!(detail_from(task, &activity).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn completed_adapter_history_preserves_relative_project_and_never_resumes_vendor_ids() {
        let record = SessionRecord {
            recorded_at_unix: 10,
            agent: "codex".into(),
            session_id: Some("vendor-session-id".into()),
            prompt: "Review the schema\nKeep existing rows".into(),
            cwd: PathBuf::from("."),
            model: Some("recorded-model".into()),
            outcome: Outcome::Success,
            final_text: "Reviewed".into(),
            usage: Usage::default(),
        };
        let task = history_task(0, &record, 20_000);
        assert_eq!(task.project, ".");
        assert_eq!(task.status, TaskStatus::Ready);
        assert_eq!(task.model, "recorded-model");
        assert_eq!(task.title, "Review the schema");
        assert!(task.resume_handle.is_none());
        assert!(!task.details.contains("vendor-session-id"));
        assert_eq!(detail(&task).unwrap(), task.details);
    }

    #[test]
    fn approval_and_reconciliation_runs_require_attention_without_executable_handles() {
        for availability in [
            SelfAgentResumeAvailability::ApprovalRequired,
            SelfAgentResumeAvailability::ReconciliationRequired,
        ] {
            let entry = SelfAgentResumeEntry {
                handle: "r-opaque".into(),
                availability,
                step: SelfAgentResumeStep::AwaitApproval,
                latest: true,
            };
            let task = resume_task(&entry, "/work/project");
            assert_eq!(task.status, TaskStatus::NeedsYou);
            assert!(task.resume_handle.is_none());
            assert_eq!(task.model, "—");
            assert_eq!(task.updated, "—");
        }
    }

    #[test]
    fn ready_runs_keep_the_opaque_resume_handle() {
        let entry = SelfAgentResumeEntry {
            handle: "r-opaque".into(),
            availability: SelfAgentResumeAvailability::Ready,
            step: SelfAgentResumeStep::StartModel,
            latest: true,
        };
        let task = resume_task(&entry, "/work/project");
        assert_eq!(task.status, TaskStatus::Ready);
        assert_eq!(task.resume_handle.as_deref(), Some("r-opaque"));
    }

    #[test]
    fn unobserved_external_state_is_inactive_and_terminal_controls_are_removed() {
        assert_eq!(external_status("unknown"), TaskStatus::Inactive);
        assert_eq!(external_status("active"), TaskStatus::Working);
        assert_eq!(external_status("completed"), TaskStatus::Ready);
        let text = safe_text("name\x1b]52;secret\x07\nnext\u{009b}31m", 100);
        assert!(!text.contains('\x1b'));
        assert!(!text.contains('\x07'));
        assert!(!text.contains('\u{009b}'));
        assert!(text.contains('\n'));
    }
}
