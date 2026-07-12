PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_versions (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS actors (
  actor_id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK(kind IN ('local_user', 'web_session')),
  subject_hash TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  expires_at TEXT
);

CREATE TABLE IF NOT EXISTS web_sessions (
  session_hash TEXT PRIMARY KEY,
  actor_id TEXT NOT NULL UNIQUE REFERENCES actors(actor_id),
  csrf_secret_hash TEXT NOT NULL,
  window_started_at TEXT NOT NULL,
  runs_in_window INTEGER NOT NULL DEFAULT 0 CHECK(runs_in_window >= 0),
  active_runs INTEGER NOT NULL DEFAULT 0 CHECK(active_runs BETWEEN 0 AND 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS projects (
  project_id TEXT PRIMARY KEY,
  canonical_root TEXT NOT NULL UNIQUE,
  workspace_identity TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runs (
  run_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id),
  owner_actor_id TEXT NOT NULL REFERENCES actors(actor_id),
  status TEXT NOT NULL,
  next_sequence INTEGER NOT NULL CHECK(next_sequence >= 1),
  current_turn_id TEXT,
  current_step_id TEXT,
  mutation_generation INTEGER NOT NULL DEFAULT 0 CHECK(mutation_generation >= 0),
  policy_snapshot_hash TEXT NOT NULL,
  config_snapshot_hash TEXT NOT NULL,
  max_steps INTEGER NOT NULL CHECK(max_steps > 0),
  steps_used INTEGER NOT NULL DEFAULT 0 CHECK(steps_used >= 0),
  max_input_tokens INTEGER,
  input_tokens_used INTEGER NOT NULL DEFAULT 0,
  max_output_tokens INTEGER,
  output_tokens_used INTEGER NOT NULL DEFAULT 0,
  deadline_at TEXT,
  row_version INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  stop_reason TEXT,
  CHECK(status IN (
    'created', 'running', 'awaiting_approval', 'validating', 'succeeded',
    'failed', 'cancelled', 'budget_exceeded', 'repeated_failure',
    'interrupted_unknown_outcome'
  ))
);

CREATE TABLE IF NOT EXISTS messages (
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  ordinal INTEGER NOT NULL,
  message_id TEXT NOT NULL UNIQUE,
  turn_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
  item_kind TEXT NOT NULL,
  call_id TEXT,
  sanitized_payload TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  PRIMARY KEY(run_id, ordinal)
);

CREATE TABLE IF NOT EXISTS context_sidecars (
  sidecar_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  source_message_count INTEGER NOT NULL CHECK(source_message_count >= 0),
  source_prefix_hash TEXT NOT NULL,
  summary TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(run_id, source_message_count, source_prefix_hash)
);

CREATE TABLE IF NOT EXISTS steps (
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  step_ordinal INTEGER NOT NULL,
  step_id TEXT NOT NULL UNIQUE,
  turn_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN (
    'created', 'model_running', 'action_recorded', 'awaiting_approval',
    'tool_running', 'observed', 'completed', 'failed', 'cancelled'
  )),
  model_call_id TEXT,
  action_id TEXT UNIQUE,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  PRIMARY KEY(run_id, step_ordinal)
);

CREATE TABLE IF NOT EXISTS actions (
  action_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  step_id TEXT NOT NULL UNIQUE REFERENCES steps(step_id),
  call_id TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,
  canonical_json TEXT NOT NULL,
  action_hash TEXT NOT NULL,
  effect_class TEXT NOT NULL CHECK(effect_class IN (
    'read_only_idempotent', 'workspace_mutation', 'may_mutate', 'external_effect'
  )),
  policy_decision TEXT CHECK(policy_decision IN ('allow', 'ask', 'deny')),
  policy_rule_id TEXT,
  state TEXT NOT NULL CHECK(state IN (
    'recorded', 'denied', 'awaiting_approval', 'ready', 'executing',
    'completed', 'failed', 'cancelled', 'interrupted'
  )),
  audit_sequence INTEGER,
  created_at TEXT NOT NULL,
  terminal_at TEXT
);

CREATE TABLE IF NOT EXISTS approvals (
  approval_id TEXT PRIMARY KEY,
  action_id TEXT NOT NULL UNIQUE REFERENCES actions(action_id),
  owner_actor_id TEXT NOT NULL REFERENCES actors(actor_id),
  state TEXT NOT NULL CHECK(state IN (
    'requested', 'awaiting', 'approved', 'denied', 'expired', 'invalidated', 'consumed'
  )),
  action_hash TEXT NOT NULL,
  workspace_identity TEXT NOT NULL,
  policy_snapshot_hash TEXT NOT NULL,
  config_snapshot_hash TEXT NOT NULL,
  risk TEXT NOT NULL,
  rule_id TEXT NOT NULL,
  decided_by_actor_id TEXT REFERENCES actors(actor_id),
  capability_nonce_hash TEXT,
  reason TEXT,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  decided_at TEXT,
  consumed_at TEXT,
  row_version INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS tool_attempts (
  call_id TEXT PRIMARY KEY,
  action_id TEXT NOT NULL REFERENCES actions(action_id),
  attempt_no INTEGER NOT NULL CHECK(attempt_no >= 1),
  state TEXT NOT NULL CHECK(state IN (
    'created', 'started', 'completed', 'failed', 'cancelled', 'interrupted'
  )),
  started_at TEXT,
  terminal_at TEXT,
  observation_id TEXT,
  UNIQUE(action_id, attempt_no)
);

CREATE TABLE IF NOT EXISTS observations (
  observation_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  step_id TEXT NOT NULL REFERENCES steps(step_id),
  call_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  sanitized_payload TEXT NOT NULL,
  fingerprint TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS validator_states (
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  validator_id TEXT NOT NULL,
  required INTEGER NOT NULL CHECK(required IN (0, 1)),
  last_passed_generation INTEGER,
  last_feedback_id TEXT REFERENCES observations(observation_id),
  updated_at TEXT NOT NULL,
  PRIMARY KEY(run_id, validator_id),
  CHECK(last_passed_generation IS NULL OR last_passed_generation >= 0)
);

CREATE TABLE IF NOT EXISTS events (
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  sequence INTEGER NOT NULL CHECK(sequence >= 1),
  schema_version INTEGER NOT NULL,
  event_id TEXT NOT NULL UNIQUE,
  turn_id TEXT,
  step_id TEXT,
  call_id TEXT,
  kind TEXT NOT NULL,
  sanitized_payload TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  PRIMARY KEY(run_id, sequence)
);

CREATE TABLE IF NOT EXISTS audit_checkpoints (
  event_id TEXT PRIMARY KEY REFERENCES events(event_id),
  audit_file TEXT NOT NULL,
  audit_sequence INTEGER NOT NULL UNIQUE,
  head_hash TEXT NOT NULL,
  synced_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_owner_updated
  ON runs(owner_actor_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_events_run_sequence
  ON events(run_id, sequence);
CREATE INDEX IF NOT EXISTS idx_approvals_state_expiry
  ON approvals(state, expires_at);
CREATE INDEX IF NOT EXISTS idx_messages_run_turn
  ON messages(run_id, turn_id, ordinal);

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(1, CURRENT_TIMESTAMP);
