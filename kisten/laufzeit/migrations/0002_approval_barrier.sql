-- Durable approval and execution-barrier fields.  This migration is applied
-- once by the runtime after 0001_state.sql; it is intentionally explicit so
-- old state databases are upgraded instead of silently recreated.

ALTER TABLE actions ADD COLUMN policy_event_id TEXT REFERENCES events(event_id);
ALTER TABLE actions ADD COLUMN audit_event_id TEXT REFERENCES events(event_id);

ALTER TABLE audit_checkpoints RENAME TO audit_checkpoints_v1;

CREATE TABLE audit_checkpoints (
  event_id TEXT PRIMARY KEY REFERENCES events(event_id),
  audit_file TEXT NOT NULL,
  audit_sequence INTEGER NOT NULL,
  head_hash TEXT NOT NULL,
  synced_at TEXT NOT NULL
);

INSERT INTO audit_checkpoints(
  event_id, audit_file, audit_sequence, head_hash, synced_at
)
SELECT event_id, audit_file, audit_sequence, head_hash, synced_at
FROM audit_checkpoints_v1;

DROP TABLE audit_checkpoints_v1;

ALTER TABLE approvals RENAME TO approvals_v1;

CREATE TABLE approvals (
  approval_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(run_id),
  action_id TEXT NOT NULL UNIQUE REFERENCES actions(action_id),
  owner_actor_id TEXT NOT NULL REFERENCES actors(actor_id),
  state TEXT NOT NULL CHECK(state IN (
    'requested', 'awaiting', 'approved', 'denied', 'expired',
    'invalidated', 'executing', 'consumed'
  )),
  action_hash TEXT NOT NULL,
  action_summary TEXT NOT NULL,
  workspace_identity TEXT NOT NULL,
  policy_snapshot_hash TEXT NOT NULL,
  config_snapshot_hash TEXT NOT NULL,
  risk TEXT NOT NULL,
  rule_id TEXT NOT NULL,
  decided_by_actor_id TEXT REFERENCES actors(actor_id),
  capability_nonce_hash TEXT,
  reason TEXT,
  created_at TEXT NOT NULL,
  created_at_unix INTEGER NOT NULL,
  expires_at TEXT NOT NULL,
  expires_at_unix INTEGER NOT NULL,
  decided_at TEXT,
  consumed_at TEXT,
  approval_event_id TEXT UNIQUE REFERENCES events(event_id),
  row_version INTEGER NOT NULL DEFAULT 0
);

INSERT INTO approvals(
  approval_id, run_id, action_id, owner_actor_id, state, action_hash,
  action_summary, workspace_identity, policy_snapshot_hash,
  config_snapshot_hash, risk, rule_id, decided_by_actor_id,
  capability_nonce_hash, reason, created_at, created_at_unix,
  expires_at, expires_at_unix, decided_at, consumed_at, approval_event_id,
  row_version
)
SELECT old.approval_id,
       action.run_id,
       old.action_id,
       old.owner_actor_id,
       old.state,
       old.action_hash,
       '',
       old.workspace_identity,
       old.policy_snapshot_hash,
       old.config_snapshot_hash,
       old.risk,
       old.rule_id,
       old.decided_by_actor_id,
       old.capability_nonce_hash,
       old.reason,
       old.created_at,
       COALESCE(unixepoch(old.created_at), 0),
       old.expires_at,
       COALESCE(unixepoch(old.expires_at), 0),
       old.decided_at,
       old.consumed_at,
       NULL,
       old.row_version
FROM approvals_v1 AS old
JOIN actions AS action ON action.action_id = old.action_id;

DROP TABLE approvals_v1;

CREATE UNIQUE INDEX idx_actions_policy_event
  ON actions(policy_event_id) WHERE policy_event_id IS NOT NULL;
CREATE UNIQUE INDEX idx_actions_audit_event
  ON actions(audit_event_id) WHERE audit_event_id IS NOT NULL;
CREATE UNIQUE INDEX idx_audit_file_sequence
  ON audit_checkpoints(audit_file, audit_sequence);
CREATE INDEX idx_approvals_run_owner
  ON approvals(run_id, owner_actor_id, state);
CREATE INDEX idx_approvals_state_expiry
  ON approvals(state, expires_at_unix);
CREATE UNIQUE INDEX idx_approvals_nonce_hash
  ON approvals(capability_nonce_hash) WHERE capability_nonce_hash IS NOT NULL;

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES(2, CURRENT_TIMESTAMP);
PRAGMA user_version = 2;
