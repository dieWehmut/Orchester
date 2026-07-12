-- Bind every terminal tool call to exactly one durable observation. Version 4
-- never wrote this table, so any pre-existing observation/pointer is ambiguous
-- and fails the migration instead of being trusted silently.

ALTER TABLE observations ADD COLUMN outcome TEXT NOT NULL DEFAULT 'completed'
  CHECK(outcome IN ('completed', 'failed', 'absent'));

CREATE TEMP TABLE migration_v5_guard (
  violations INTEGER NOT NULL CHECK(violations = 0)
);

INSERT INTO migration_v5_guard(violations)
SELECT COUNT(*) FROM observations;

INSERT INTO migration_v5_guard(violations)
SELECT COUNT(*) FROM tool_attempts WHERE observation_id IS NOT NULL;

INSERT INTO migration_v5_guard(violations)
SELECT COUNT(*)
FROM tool_attempts AS attempt
JOIN actions AS action ON action.action_id = attempt.action_id
JOIN steps AS step
  ON step.run_id = action.run_id AND step.step_id = action.step_id
WHERE attempt.call_id != action.call_id
   OR step.action_id IS NULL
   OR step.action_id != action.action_id;

DROP TABLE migration_v5_guard;

CREATE UNIQUE INDEX idx_observations_call
  ON observations(call_id);

CREATE UNIQUE INDEX idx_observations_id_call
  ON observations(observation_id, call_id);

INSERT INTO observations(
  observation_id, run_id, step_id, call_id, kind, sanitized_payload,
  fingerprint, created_at, outcome
)
SELECT 'legacy-observation:' || attempt.call_id,
       action.run_id,
       action.step_id,
       attempt.call_id,
       'tool.absent',
       '{"reason":"legacy_unrecorded"}',
       NULL,
       COALESCE(attempt.terminal_at, attempt.started_at, CURRENT_TIMESTAMP),
       'absent'
FROM tool_attempts AS attempt
JOIN actions AS action ON action.action_id = attempt.action_id
WHERE attempt.state IN ('completed', 'failed', 'cancelled', 'interrupted');

ALTER TABLE tool_attempts RENAME TO tool_attempts_v4;

CREATE TABLE tool_attempts (
  call_id TEXT PRIMARY KEY,
  action_id TEXT NOT NULL REFERENCES actions(action_id),
  attempt_no INTEGER NOT NULL CHECK(attempt_no >= 1),
  state TEXT NOT NULL CHECK(state IN (
    'created', 'started', 'completed', 'failed', 'cancelled', 'interrupted'
  )),
  started_at TEXT,
  terminal_at TEXT,
  observation_id TEXT,
  UNIQUE(action_id, attempt_no),
  FOREIGN KEY(observation_id, call_id)
    REFERENCES observations(observation_id, call_id),
  CHECK(
    (state IN ('created', 'started') AND observation_id IS NULL)
    OR
    (state IN ('completed', 'failed', 'cancelled', 'interrupted')
      AND observation_id IS NOT NULL)
  )
);

INSERT INTO tool_attempts(
  call_id, action_id, attempt_no, state, started_at, terminal_at,
  observation_id
)
SELECT call_id,
       action_id,
       attempt_no,
       state,
       started_at,
       terminal_at,
       CASE
         WHEN state IN ('completed', 'failed', 'cancelled', 'interrupted')
         THEN 'legacy-observation:' || call_id
         ELSE NULL
       END
FROM tool_attempts_v4;

DROP TABLE tool_attempts_v4;

CREATE UNIQUE INDEX idx_tool_attempts_observation
  ON tool_attempts(observation_id) WHERE observation_id IS NOT NULL;

CREATE TRIGGER trg_observations_validate_tool_insert
BEFORE INSERT ON observations
WHEN NEW.kind LIKE 'tool.%' AND (
  NEW.kind NOT IN ('tool.completed', 'tool.failed', 'tool.absent')
  OR json_valid(NEW.sanitized_payload) != 1
  OR length(CAST(NEW.sanitized_payload AS BLOB)) > 65536
  OR NEW.fingerprint IS NULL
  OR length(NEW.fingerprint) != 64
  OR NEW.fingerprint GLOB '*[^0-9a-f]*'
  OR (NEW.kind = 'tool.completed' AND NEW.outcome != 'completed')
  OR (NEW.kind = 'tool.failed' AND NEW.outcome != 'failed')
  OR (NEW.kind = 'tool.absent' AND NEW.outcome != 'absent')
  OR NOT EXISTS (
    SELECT 1
    FROM tool_attempts AS attempt
    JOIN actions AS action ON action.action_id = attempt.action_id
    JOIN steps AS step
      ON step.run_id = action.run_id AND step.step_id = action.step_id
    WHERE attempt.call_id = NEW.call_id
      AND attempt.state = 'started'
      AND action.call_id = attempt.call_id
      AND action.run_id = NEW.run_id
      AND action.step_id = NEW.step_id
      AND action.state = 'executing'
      AND step.action_id = action.action_id
      AND step.status = 'tool_running'
  )
)
BEGIN
  SELECT RAISE(ABORT, 'invalid tool observation');
END;

CREATE TRIGGER trg_tool_attempts_validate_observation_insert
BEFORE INSERT ON tool_attempts
WHEN NEW.observation_id IS NOT NULL AND NOT EXISTS (
  SELECT 1
  FROM observations AS observation
  JOIN actions AS action ON action.action_id = NEW.action_id
  JOIN steps AS step
    ON step.run_id = action.run_id AND step.step_id = action.step_id
  WHERE observation.observation_id = NEW.observation_id
    AND observation.call_id = NEW.call_id
    AND observation.run_id = action.run_id
    AND observation.step_id = action.step_id
    AND action.call_id = NEW.call_id
    AND step.action_id = action.action_id
    AND (
      (NEW.state = 'completed' AND observation.kind = 'tool.completed'
        AND observation.outcome = 'completed')
      OR
      (NEW.state = 'failed' AND observation.kind = 'tool.failed'
        AND observation.outcome = 'failed')
      OR
      (NEW.state IN ('cancelled', 'interrupted')
        AND observation.kind = 'tool.absent'
        AND observation.outcome = 'absent')
    )
)
BEGIN
  SELECT RAISE(ABORT, 'tool attempt observation binding mismatch');
END;

CREATE TRIGGER trg_tool_attempts_validate_observation_update
BEFORE UPDATE OF state, observation_id ON tool_attempts
WHEN NEW.observation_id IS NOT NULL AND NOT EXISTS (
  SELECT 1
  FROM observations AS observation
  JOIN actions AS action ON action.action_id = NEW.action_id
  JOIN steps AS step
    ON step.run_id = action.run_id AND step.step_id = action.step_id
  WHERE observation.observation_id = NEW.observation_id
    AND observation.call_id = NEW.call_id
    AND observation.run_id = action.run_id
    AND observation.step_id = action.step_id
    AND action.call_id = NEW.call_id
    AND step.action_id = action.action_id
    AND (
      (NEW.state = 'completed' AND observation.kind = 'tool.completed'
        AND observation.outcome = 'completed')
      OR
      (NEW.state = 'failed' AND observation.kind = 'tool.failed'
        AND observation.outcome = 'failed')
      OR
      (NEW.state IN ('cancelled', 'interrupted')
        AND observation.kind = 'tool.absent'
        AND observation.outcome = 'absent')
    )
)
BEGIN
  SELECT RAISE(ABORT, 'tool attempt observation binding mismatch');
END;

CREATE TRIGGER trg_observations_no_update
BEFORE UPDATE ON observations
BEGIN
  SELECT RAISE(ABORT, 'durable observations are append-only');
END;

CREATE TRIGGER trg_observations_no_delete
BEFORE DELETE ON observations
BEGIN
  SELECT RAISE(ABORT, 'durable observations are append-only');
END;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(5, CURRENT_TIMESTAMP);
PRAGMA user_version = 5;
