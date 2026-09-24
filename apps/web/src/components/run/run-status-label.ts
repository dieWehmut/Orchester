import { RUN_STATUS_LABEL_KEYS, RUN_STATUSES, type RunStatus } from '@orchester/ereignis'

/**
 * What a run's state is called, in the catalogue's own keys.
 *
 * The ereignis package names the keys a state maps to and leaves localization to
 * the host; this is the host's half of that contract, written as the catalogue's
 * keys so the compiler refuses a key the catalogues do not have. The two maps
 * are asserted against each other in `run-status-label.test.ts`, which is what
 * keeps a state added to the model from arriving here without a label.
 */
export type RunStatusMessageKey =
  | 'run.status.idle'
  | 'run.status.running'
  | 'run.status.succeeded'
  | 'run.status.failed'
  | 'run.status.cancelled'
  | 'run.status.awaiting_approval'
  | 'run.status.budget_exceeded'
  | 'run.status.repeated_failure'
  | 'run.status.interrupted_unknown_outcome'

export const RUN_STATUS_MESSAGE_KEYS = {
  idle: 'run.status.idle',
  running: 'run.status.running',
  succeeded: 'run.status.succeeded',
  failed: 'run.status.failed',
  cancelled: 'run.status.cancelled',
  awaiting_approval: 'run.status.awaiting_approval',
  budget_exceeded: 'run.status.budget_exceeded',
  repeated_failure: 'run.status.repeated_failure',
  interrupted_unknown_outcome: 'run.status.interrupted_unknown_outcome',
} as const satisfies Record<RunStatus, RunStatusMessageKey>

/** Every state the model can report has a key here. */
export function runStatusMessageKey(status: RunStatus): RunStatusMessageKey {
  return RUN_STATUS_MESSAGE_KEYS[status]
}

/** The states, for a test that has to walk them. */
export const RUN_STATUS_VALUES = RUN_STATUSES

/** The model's own map, so a test can prove the two agree. */
export const RUN_STATUS_MODEL_KEYS = RUN_STATUS_LABEL_KEYS