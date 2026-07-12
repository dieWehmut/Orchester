-- Persist the model request phase independently from the broader step status.
-- This lets recovery distinguish a model call that is still in flight from a
-- completed call whose tool action has not yet been recorded.

ALTER TABLE steps ADD COLUMN model_phase TEXT NOT NULL DEFAULT 'not_started'
  CHECK(model_phase IN ('not_started', 'running', 'completed'));

UPDATE steps
SET model_phase = CASE
  WHEN model_call_id IS NULL THEN 'not_started'
  WHEN EXISTS (
    SELECT 1
    FROM events
    WHERE events.run_id = steps.run_id
      AND events.step_id = steps.step_id
      AND events.kind = 'model.completed'
      AND events.call_id = steps.model_call_id
  ) THEN 'completed'
  ELSE 'running'
END;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(3, CURRENT_TIMESTAMP);
PRAGMA user_version = 3;
