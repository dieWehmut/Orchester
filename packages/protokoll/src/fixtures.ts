import {
  callId,
  eventId,
  runId,
  turnId,
  UI_SCHEMA_VERSION,
  type UiEventEnvelope,
  type UiEventKind,
} from './ui'

const FIXTURE_RUN_ID = runId('run-fixture')
const FIXTURE_TURN_ID = turnId('turn-fixture')
const FIXTURE_EPOCH = Date.parse('2026-08-19T00:00:00Z')

/** Build one deterministic event for tests, demos, and contract fixtures. */
export function fixtureEnvelope(sequence: number, kind: UiEventKind): UiEventEnvelope {
  if (!Number.isSafeInteger(sequence) || sequence <= 0) {
    throw new RangeError('fixture sequence must be a positive safe integer')
  }
  const envelope: UiEventEnvelope = {
    schema_version: UI_SCHEMA_VERSION,
    event_id: eventId(`event-fixture-${sequence}`),
    run_id: FIXTURE_RUN_ID,
    turn_id: FIXTURE_TURN_ID,
    sequence,
    occurred_at: new Date(FIXTURE_EPOCH + (sequence - 1) * 1_000).toISOString(),
    kind,
  }
  return kind.type === 'tool_call' ? { ...envelope, call_id: callId(kind.call_id) } : envelope
}

export function toFixtureJsonLines(events: readonly UiEventEnvelope[]): string[] {
  return events.map((event) => JSON.stringify(event))
}
