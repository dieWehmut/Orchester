CREATE TABLE memory_schema_versions (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

INSERT INTO memory_schema_versions(version, applied_at)
SELECT version, applied_at FROM schema_versions;

INSERT INTO memory_schema_versions(version, applied_at)
VALUES(4, CURRENT_TIMESTAMP);

DROP TABLE schema_versions;
