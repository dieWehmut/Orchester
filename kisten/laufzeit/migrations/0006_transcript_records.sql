-- Store canonical provider-neutral transcript records for restart and resume.

CREATE TABLE transcript_records (
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  ordinal INTEGER NOT NULL CHECK(ordinal >= 1),
  kind TEXT NOT NULL CHECK(kind IN (
    'system', 'user', 'assistant', 'tool_call', 'tool_result', 'opaque'
  )),
  call_id TEXT,
  wire_json TEXT NOT NULL CHECK(
    json_valid(wire_json) = 1
    AND length(CAST(wire_json AS BLOB)) <= 65536
  ),
  record_hash TEXT NOT NULL CHECK(
    length(record_hash) = 64
    AND record_hash NOT GLOB '*[^0-9a-f]*'
  ),
  created_at TEXT NOT NULL,
  PRIMARY KEY(run_id, ordinal)
);

CREATE INDEX idx_transcript_records_run_ordinal
  ON transcript_records(run_id, ordinal);

CREATE TRIGGER trg_transcript_records_no_update
BEFORE UPDATE ON transcript_records
BEGIN
  SELECT RAISE(ABORT, 'durable transcript records are append-only');
END;

CREATE TRIGGER trg_transcript_records_no_delete
BEFORE DELETE ON transcript_records
BEGIN
  SELECT RAISE(ABORT, 'durable transcript records are append-only');
END;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(6, CURRENT_TIMESTAMP);
PRAGMA user_version = 6;
