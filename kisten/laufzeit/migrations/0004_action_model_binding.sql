-- Preserve the model request that produced each durable action.
-- Legacy rows are backfilled only when their step has a durable model call.

ALTER TABLE actions ADD COLUMN origin_model_call_id TEXT;

UPDATE actions
SET origin_model_call_id = (
  SELECT steps.model_call_id
  FROM steps
  WHERE steps.run_id = actions.run_id
    AND steps.step_id = actions.step_id
)
WHERE origin_model_call_id IS NULL;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(4, CURRENT_TIMESTAMP);
PRAGMA user_version = 4;
