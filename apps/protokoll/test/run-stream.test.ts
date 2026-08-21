import { describe, expect, it } from 'vitest'

import {
  parseRunStreamFrame,
  runId,
  type UiEventEnvelope,
} from '../src/index'

const event: UiEventEnvelope = {
  schema_version: 1,
  event_id: 'event-1' as UiEventEnvelope['event_id'],
  run_id: runId('run-1'),
  sequence: 3,
  occurred_at: '2026-08-20T00:00:00.000Z',
  kind: { type: 'message', text: 'hello' },
}

describe('run stream frame guard', () => {
  it('accepts a validated event frame', () => {
    const frame = parseRunStreamFrame({ type: 'event', event })
    expect(frame).toEqual({ type: 'event', event })
  })

  it('accepts a resync request with bounded sequence metadata', () => {
    const frame = parseRunStreamFrame({
      type: 'resync_required',
      run_id: runId('run-1'),
      requested_after_sequence: 2,
      oldest_sequence: 3,
      latest_sequence: 9,
      reason: 'sequence_gap',
    })
    expect(frame?.type).toBe('resync_required')
  })

  it.each([
    { type: 'event', event: { ...event, sequence: 0 } },
    { type: 'event', event: { ...event, run_id: runId('') } },
    {
      type: 'resync_required',
      run_id: runId('run-1'),
      requested_after_sequence: 9,
      oldest_sequence: 3,
      latest_sequence: 4,
      reason: 'sequence_gap',
    },
    {
      type: 'resync_required',
      run_id: runId('run-1'),
      requested_after_sequence: 2,
      oldest_sequence: 3,
      latest_sequence: 9,
      reason: 'nope',
    },
    { type: 'unknown' },
  ] satisfies readonly unknown[])('rejects malformed frame %#', (candidate) => {
    expect(parseRunStreamFrame(candidate)).toBeNull()
  })

  it('does not accept extra frame keys', () => {
    const candidate = { type: 'event', event, debug: true } satisfies Record<string, unknown>
    expect(parseRunStreamFrame(candidate)).toBeNull()
  })
})
