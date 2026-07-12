PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_versions (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_items (
  memory_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('proposed', 'accepted', 'rejected', 'forgotten')),
  kind TEXT NOT NULL CHECK(kind IN ('convention', 'architecture_decision', 'lesson')),
  content TEXT NOT NULL,
  content_hash TEXT,
  source_run_id TEXT,
  source TEXT NOT NULL,
  confidence REAL NOT NULL CHECK(confidence >= 0.0 AND confidence <= 1.0),
  created_at TEXT NOT NULL,
  decided_at TEXT,
  decided_by_actor_id TEXT,
  row_version INTEGER NOT NULL DEFAULT 0 CHECK(row_version >= 0),
  CHECK(
    (state = 'forgotten' AND content = '' AND content_hash IS NULL)
    OR (state <> 'forgotten' AND length(content) > 0 AND content_hash IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_memory_scope_state_kind
  ON memory_items(project_id, state, kind);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
  memory_id UNINDEXED,
  project_id UNINDEXED,
  content,
  tokenize = 'unicode61'
);

CREATE TRIGGER IF NOT EXISTS memory_fts_insert
AFTER INSERT ON memory_items
WHEN NEW.state = 'accepted'
BEGIN
  INSERT INTO memory_fts(memory_id, project_id, content)
  VALUES(NEW.memory_id, NEW.project_id, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS memory_fts_update
AFTER UPDATE OF state, content ON memory_items
BEGIN
  DELETE FROM memory_fts WHERE memory_id = OLD.memory_id;
  INSERT INTO memory_fts(memory_id, project_id, content)
    SELECT NEW.memory_id, NEW.project_id, NEW.content
    WHERE NEW.state = 'accepted';
END;

CREATE TRIGGER IF NOT EXISTS memory_fts_delete
AFTER DELETE ON memory_items
BEGIN
  DELETE FROM memory_fts WHERE memory_id = OLD.memory_id;
END;

CREATE TABLE IF NOT EXISTS memory_events (
  event_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE,
  memory_id TEXT NOT NULL REFERENCES memory_items(memory_id),
  project_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('proposed', 'approved', 'rejected', 'forgotten')),
  content_hash TEXT,
  actor_id TEXT,
  occurred_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_events_project_sequence
  ON memory_events(project_id, event_sequence);

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(1, CURRENT_TIMESTAMP);
