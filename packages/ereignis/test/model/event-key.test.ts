import {
  UI_EVENT_TYPES,
  approvalId,
  callId,
  fixtureEnvelope,
  type UiEventEnvelope,
  type UiEventKind,
} from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { eventKey, gapKey, timelineItemKey } from '../../src/model/event-key'

const kinds = {
  run_started: { type: 'run_started' },
  turn_started: { type: 'turn_started' },
  message: { type: 'message', text: 'done' },
  message_delta: { type: 'message_delta', text: 'part', final: false },
  reasoning: { type: 'reasoning', text: 'digest' },
  tool_call: { type: 'tool_call', call_id: callId('call-1'), name: 'read', state: 'running' },
  file_change: { type: 'file_change', path: 'src/main.ts', kind: 'update' },
  todo_list: { type: 'todo_list', items: [] },
  usage: {
    type: 'usage',
    input_tokens: 1,
    output_tokens: 2,
    cached_input_tokens: 3,
    reasoning_output_tokens: 4,
  },
  approval_requested: {
    type: 'approval_requested',
    approval: {
      approval_id: approvalId('approval-1'),
      run_id: fixtureEnvelope(1, { type: 'run_started' }).run_id,
      row_version: 1,
      risk: 'write',
      action: 'write source',
      reason: 'modifies source',
    },
  },
  approval_resolved: {
    type: 'approval_resolved',
    resolution: {
      approval_id: approvalId('approval-1'),
      row_version: 2,
      decision: 'approved',
    },
  },
  validation: { type: 'validation', validation: { ok: true, summary: 'passed' } },
  run_stopped: { type: 'run_stopped', reason: 'succeeded' },
  error: { type: 'error', code: 'failed', message: 'failed safely' },
} satisfies Record<(typeof UI_EVENT_TYPES)[number], UiEventKind>

describe('event keys', () => {
  it('deduplicates by run and sequence rather than event payload', () => {
    const event = fixtureEnvelope(7, { type: 'message', text: 'first payload' })
    const duplicate = {
      ...event,
      kind: { type: 'message', text: 'replacement payload' },
    } satisfies UiEventEnvelope

    expect(eventKey(event)).toBe(eventKey(duplicate))
    expect(eventKey(event)).toBe('run-fixture:7')
  })

  it('routes every event kind to a stable timeline key', () => {
    const keys = UI_EVENT_TYPES.map((type, index) =>
      timelineItemKey(fixtureEnvelope(index + 1, kinds[type])),
    )

    expect(keys).toHaveLength(UI_EVENT_TYPES.length)
    expect(keys[UI_EVENT_TYPES.indexOf('tool_call')]).toBe('tool:call-1')
    expect(keys[UI_EVENT_TYPES.indexOf('approval_requested')]).toBe('approval:approval-1')
    expect(keys[UI_EVENT_TYPES.indexOf('approval_resolved')]).toBe('approval:approval-1')
  })

  it('keys a missing sequence range independently of connection state', () => {
    expect(gapKey('run-fixture', 3, 5)).toBe('gap:run-fixture:3-5')
  })
})
