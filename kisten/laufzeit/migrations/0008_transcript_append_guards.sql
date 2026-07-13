-- Prevent SQLite replacement conflict handling from bypassing append-only
-- transcript and lifecycle-binding triggers.

CREATE TRIGGER trg_transcript_records_no_replace
BEFORE INSERT ON transcript_records
WHEN EXISTS(
  SELECT 1 FROM transcript_records
  WHERE run_id = NEW.run_id AND ordinal = NEW.ordinal
)
BEGIN
  SELECT RAISE(ABORT, 'durable transcript records are append-only');
END;

CREATE TRIGGER trg_transcript_bindings_no_replace
BEFORE INSERT ON transcript_bindings
WHEN EXISTS(
  SELECT 1 FROM transcript_bindings
  WHERE run_id = NEW.run_id
    AND event_sequence = NEW.event_sequence
    AND phase = NEW.phase
)
BEGIN
  SELECT RAISE(ABORT, 'transcript bindings are append-only');
END;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(8, CURRENT_TIMESTAMP);
PRAGMA user_version = 8;
