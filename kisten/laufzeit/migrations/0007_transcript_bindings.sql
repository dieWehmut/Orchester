-- Bind durable transcript ranges to the lifecycle event that produced them.

CREATE TABLE transcript_bindings (
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  event_sequence INTEGER NOT NULL CHECK(event_sequence >= 1),
  phase TEXT NOT NULL CHECK(phase IN (
    'model_request', 'model_response', 'action', 'tool_result'
  )),
  first_ordinal INTEGER,
  last_ordinal INTEGER,
  record_count INTEGER NOT NULL CHECK(record_count >= 0),
  PRIMARY KEY(run_id, event_sequence, phase),
  FOREIGN KEY(run_id, event_sequence) REFERENCES events(run_id, sequence),
  FOREIGN KEY(run_id, first_ordinal) REFERENCES transcript_records(run_id, ordinal),
  FOREIGN KEY(run_id, last_ordinal) REFERENCES transcript_records(run_id, ordinal),
  CHECK(first_ordinal IS NULL OR first_ordinal >= 1),
  CHECK(last_ordinal IS NULL OR last_ordinal >= 1),
  CHECK(
    (record_count = 0 AND first_ordinal IS NULL AND last_ordinal IS NULL)
    OR (record_count > 0 AND first_ordinal IS NOT NULL AND last_ordinal IS NOT NULL
        AND last_ordinal = first_ordinal + record_count - 1)
  )
);

CREATE INDEX idx_transcript_bindings_run_first
  ON transcript_bindings(run_id, first_ordinal);

CREATE TRIGGER trg_transcript_bindings_no_update
BEFORE UPDATE ON transcript_bindings
BEGIN
  SELECT RAISE(ABORT, 'transcript bindings are append-only');
END;

CREATE TRIGGER trg_transcript_bindings_no_delete
BEFORE DELETE ON transcript_bindings
BEGIN
  SELECT RAISE(ABORT, 'transcript bindings are append-only');
END;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(7, CURRENT_TIMESTAMP);
PRAGMA user_version = 7;
