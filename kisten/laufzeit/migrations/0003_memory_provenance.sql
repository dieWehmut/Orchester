ALTER TABLE memory_items
  ADD COLUMN proposed_by_actor_id TEXT NOT NULL DEFAULT 'legacy';

CREATE TABLE memory_runs (
  run_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  owner_actor_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_memory_runs_project ON memory_runs(project_id);

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(3, CURRENT_TIMESTAMP);
