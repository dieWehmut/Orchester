import { describe, expect, it } from 'vitest'

import {
  callId,
  fixtureEnvelope,
  parseUiEventEnvelope,
  toFixtureJsonLines,
} from '../../src/index'

describe('fixtureEnvelope', () => {
  it('creates deterministic, parser-valid envelopes', () => {
    const first = fixtureEnvelope(1, { type: 'turn_started' })
    const repeated = fixtureEnvelope(1, { type: 'turn_started' })

    expect(repeated).toEqual(first)
    expect(parseUiEventEnvelope(first)).toEqual(first)
  })

  it('copies a tool call id into the outer envelope', () => {
    const event = fixtureEnvelope(2, {
      type: 'tool_call',
      call_id: callId('call-fixture'),
      name: 'read_file',
      state: 'running',
    })

    expect(event.call_id).toBe('call-fixture')
    expect(parseUiEventEnvelope(event)).toEqual(event)
  })

  it('serializes one JSONL frame per event', () => {
    const lines = toFixtureJsonLines([
      fixtureEnvelope(1, { type: 'turn_started' }),
      fixtureEnvelope(2, { type: 'message', text: 'Ready' }),
    ])

    expect(lines).toHaveLength(2)
    expect(lines.every((line) => parseUiEventEnvelope(JSON.parse(line)) !== null)).toBe(true)
  })
})
