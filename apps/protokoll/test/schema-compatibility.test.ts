import { describe, expect, it } from 'vitest'

import {
  LEGACY_EVENT_SCHEMA_VERSION,
  UI_SCHEMA_VERSION,
  parseEvent,
  parseUiEventEnvelope,
} from '../src/index'

const legacyMessage = {
  type: 'message',
  text: 'legacy flat event',
}

const uiMessage = {
  schema_version: UI_SCHEMA_VERSION,
  event_id: 'event-ui1',
  run_id: 'run-1',
  turn_id: 'turn-1',
  sequence: 1,
  occurred_at: '2026-08-19T00:00:00Z',
  kind: {
    type: 'message',
    text: 'browser envelope',
  },
}

describe('protocol schema compatibility', () => {
  it('keeps flat legacy events on the unversioned compatibility path', () => {
    expect(LEGACY_EVENT_SCHEMA_VERSION).toBe(0)
    expect(parseEvent(legacyMessage)).toEqual(legacyMessage)
    expect(parseUiEventEnvelope(legacyMessage)).toBeNull()
  })

  it('accepts browser UI v1 without routing it through the legacy parser', () => {
    expect(UI_SCHEMA_VERSION).toBe(1)
    expect(parseUiEventEnvelope(uiMessage)).toEqual(uiMessage)
    expect(parseEvent(uiMessage)).toBeNull()
  })

  it('rejects adjacent browser schema versions instead of guessing', () => {
    expect(parseUiEventEnvelope({ ...uiMessage, schema_version: 0 })).toBeNull()
    expect(parseUiEventEnvelope({ ...uiMessage, schema_version: 2 })).toBeNull()
  })
})
