CREATE TABLE IF NOT EXISTS memory_projects (
  project_id TEXT PRIMARY KEY,
  owner_actor_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_projects_owner
  ON memory_projects(owner_actor_id);

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(2, CURRENT_TIMESTAMP);
