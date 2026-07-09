use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_protokoll::{Outcome, RunResult, Task, Usage};
use serde::{Deserialize, Serialize};

/// A durable summary of one completed Orchester run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub recorded_at_unix: u64,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub prompt: String,
    pub cwd: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub outcome: Outcome,
    pub final_text: String,
    pub usage: Usage,
}

impl SessionRecord {
    pub fn new(agent: impl Into<String>, task: &Task, result: &RunResult) -> Self {
        Self {
            recorded_at_unix: now_unix(),
            agent: agent.into(),
            session_id: result.session_id.clone(),
            prompt: task.prompt.clone(),
            cwd: task.cwd.clone(),
            model: task.model.clone(),
            outcome: result.outcome,
            final_text: result.final_text.clone(),
            usage: result.usage,
        }
    }
}

/// JSONL-backed session metadata store.
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, record: &SessionRecord) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, record).map_err(invalid_data)?;
        writeln!(file)?;
        Ok(())
    }

    pub fn load(&self) -> io::Result<Vec<SessionRecord>> {
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let mut records = Vec::new();
        for line in io::BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(serde_json::from_str(&line).map_err(invalid_data)?);
        }
        Ok(records)
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn invalid_data(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "orchester-session-store-{name}-{}-{}.jsonl",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn sample_record(prompt: &str) -> SessionRecord {
        let task = Task::new(prompt, PathBuf::from(".")).with_model("test-model");
        let result = RunResult {
            session_id: Some("sid".into()),
            final_text: "done".into(),
            usage: Usage {
                input_tokens: 3,
                output_tokens: 5,
                ..Usage::default()
            },
            outcome: Outcome::Success,
        };
        SessionRecord::new("mock", &task, &result)
    }

    #[test]
    fn missing_store_loads_empty() {
        let path = temp_file("missing");
        let store = SessionStore::new(&path);
        assert_eq!(store.load().unwrap(), Vec::new());
    }

    #[test]
    fn append_and_load_roundtrip() {
        let path = temp_file("roundtrip");
        let store = SessionStore::new(&path);
        let first = sample_record("first");
        let second = sample_record("second");

        store.append(&first).unwrap();
        store.append(&second).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded, vec![first, second]);

        let _ = fs::remove_file(path);
    }
}
