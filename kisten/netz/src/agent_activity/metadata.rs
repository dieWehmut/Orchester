//! Optional, read-only Codex title/index hints. Transcript roots remain authoritative.
use super::{ActivityDetail, clip, opaque_id};
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct Hint {
    title: String,
    project: String,
    project_id: String,
    pinned: Option<bool>,
    updated_at: i64,
    preview: String,
}

impl Hint {
    pub fn pinned(&self) -> bool {
        self.pinned == Some(true)
    }
    pub fn apply(&self, row: &mut ActivityDetail) {
        if !self.title.is_empty() {
            row.summary.title = self.title.clone();
        }
        if !self.project.is_empty() {
            row.summary.project = self.project.clone();
        }
        row.summary.project_id = Some(self.project_id.clone());
        row.summary.pinned = self.pinned;
        row.summary.updated_at = self.updated_at;
        if row.summary.preview.is_empty() {
            row.summary.preview = self.preview.clone();
        }
    }
}

pub(super) fn codex(root: &Path) -> Option<HashMap<PathBuf, Hint>> {
    let parent = root.parent()?;
    let mut databases = fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name
                .strip_prefix("state_")?
                .strip_suffix(".sqlite")?
                .parse::<u32>()
                .ok()?;
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            (!super::files::linked(&metadata) && metadata.is_file())
                .then_some((version, entry.path()))
        })
        .collect::<Vec<_>>();
    databases.sort_by_key(|item| std::cmp::Reverse(item.0));
    for (_, path) in databases.into_iter().take(3) {
        if let Ok(hints) = read(&path, root) {
            return Some(hints);
        }
    }
    None
}

fn read(path: &Path, root: &Path) -> rusqlite::Result<HashMap<PathBuf, Hint>> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_millis(150))?;
    db.pragma_update(None, "query_only", true)?;
    let columns = db
        .prepare("PRAGMA table_info(threads)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<HashSet<_>>>()?;
    let pinned = if columns.contains("is_pinned") {
        "is_pinned"
    } else {
        "NULL"
    };
    let project_id = if columns.contains("project_id") {
        "project_id"
    } else {
        "NULL"
    };
    let source_filter = if columns.contains("agent_role") {
        "AND agent_role IS NULL"
    } else {
        ""
    };
    let title = if columns.contains("name") {
        "COALESCE(NULLIF(name, ''), title)"
    } else {
        "title"
    };
    let legacy_pins = read_legacy_pins(root);
    let updated = if columns.contains("updated_at_ms") {
        "COALESCE(updated_at_ms, updated_at * 1000)"
    } else {
        "updated_at * 1000"
    };
    let preview = if columns.contains("preview") {
        "COALESCE(preview, '')"
    } else {
        "''"
    };
    // All interpolated identifiers are constants selected above, never source values.
    let sql = format!(
        "SELECT rollout_path, {title}, cwd, {project_id}, {pinned}, id, {updated}, {preview} FROM threads WHERE archived = 0 {source_filter} ORDER BY updated_at DESC, id DESC"
    );
    let mut query = db.prepare(&sql)?;
    let rows = query.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<bool>>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, String>(7)?,
        ))
    })?;
    let mut hints = HashMap::new();
    for item in rows {
        let (path, title, cwd, project_id, pinned, id, updated_at, preview) = item?;
        let pinned = if legacy_pins.contains(&id) {
            Some(true)
        } else {
            pinned
        };
        let mut path = PathBuf::from(path);
        if !path.exists() && path.extension().is_some_and(|ext| ext == "jsonl") {
            path = path.with_extension("jsonl.zst");
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if super::files::linked(&metadata) || !metadata.is_file() {
            continue;
        }
        let Ok(path) = path.canonicalize() else {
            continue;
        };
        if !path.starts_with(root) || !super::files::is_transcript(&path) {
            continue;
        }
        let project_name = project_id.as_ref().and_then(|id| {
            db.query_row("SELECT name FROM projects WHERE id = ?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .ok()
        });
        hints.insert(
            path,
            Hint {
                title: clip(title.trim(), 120),
                project: project_name
                    .map(|name| clip(&name, 120))
                    .unwrap_or_else(|| super::transcript::project(&cwd)),
                project_id: opaque_id("codex-project", project_id.as_deref().unwrap_or(&cwd)),
                pinned,
                updated_at,
                preview: clip(&preview, 200),
            },
        );
    }
    Ok(hints)
}

fn read_legacy_pins(root: &Path) -> HashSet<String> {
    let Some(parent) = root.parent() else {
        return HashSet::new();
    };
    let path = parent.join(".codex-global-state.json");
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return HashSet::new();
    };
    if super::files::linked(&metadata) || !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
        return HashSet::new();
    }
    let mut bytes = Vec::new();
    let Ok(file) = fs::File::open(path) else {
        return HashSet::new();
    };
    if file.take(2 * 1024 * 1024).read_to_end(&mut bytes).is_err() {
        return HashSet::new();
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return HashSet::new();
    };
    value["pinned-thread-ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|s| s.len() <= 100)
        .map(str::to_owned)
        .collect()
}
