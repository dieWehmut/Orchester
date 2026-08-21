import { describe, expect, it } from 'vitest'

import {
  UI_EVENT_TYPES,
  UI_SCHEMA_VERSION,
  UI_TOOL_STATES,
  approvalId,
  callId,
  eventId,
  runId,
  turnId,
  type UiEventEnvelope,
  type UiEventKind,
  type UiEventType,
} from '../src/index'

const SAMPLES: Record<UiEventType, UiEventKind> = {
  run_started: { type: 'run_started', title: 'Protocol mirror' },
  turn_started: { type: 'turn_started' },
  message: { type: 'message', text: 'Ready' },
  message_delta: { type: 'message_delta', text: 'Streaming', final: false },
  reasoning: { type: 'reasoning', text: 'Inspecting the workspace' },
  tool_call: {
    type: 'tool_call',
    call_id: callId('call-1'),
    name: 'read_file',
    state: 'succeeded',
    detail: 'src/main.rs',
  },
  file_change: { type: 'file_change', path: 'src/main.rs', kind: 'update' },
  todo_list: { type: 'todo_list', items: [{ text: 'Verify', completed: false }] },
  usage: {
    type: 'usage',
    input_tokens: 10,
    output_tokens: 20,
    cached_input_tokens: 5,
    reasoning_output_tokens: 2,
  },
  approval_requested: {
    type: 'approval_requested',
    approval: {
      approval_id: approvalId('approval-1'),
      run_id: runId('run-1'),
      row_version: 1,
      risk: 'high',
      action: 'write_file path=src/main.rs',
      reason: 'workspace write',
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
  validation: {
    type: 'validation',
    validation: { ok: true, summary: 'Checks passed' },
  },
  run_stopped: { type: 'run_stopped', reason: 'succeeded' },
  error: { type: 'error', code: 'runtime_error', message: 'Run failed' },
}

describe('the browser UI event mirror', () => {
  it('covers every Rust event kind exactly once', () => {
    expect([...UI_EVENT_TYPES].sort()).toEqual(Object.keys(SAMPLES).sort())
    expect(UI_EVENT_TYPES).toHaveLength(14)
  })

  it('pins every tool lifecycle wire value', () => {
    expect(UI_TOOL_STATES).toEqual(['queued', 'running', 'succeeded', 'failed', 'cancelled'])
  })

  it('keeps the nested kind and correlation identifiers on the wire', () => {
    const envelope: UiEventEnvelope = {
      schema_version: UI_SCHEMA_VERSION,
      event_id: eventId('event-1'),
      run_id: runId('run-1'),
      turn_id: turnId('turn-1'),
      call_id: callId('call-1'),
      sequence: 7,
      occurred_at: '2026-08-19T00:00:00Z',
      kind: SAMPLES.tool_call,
    }

    expect(JSON.parse(JSON.stringify(envelope))).toEqual({
      schema_version: 1,
      event_id: 'event-1',
      run_id: 'run-1',
      turn_id: 'turn-1',
      call_id: 'call-1',
      sequence: 7,
      occurred_at: '2026-08-19T00:00:00Z',
      kind: {
        type: 'tool_call',
        call_id: 'call-1',
        name: 'read_file',
        state: 'succeeded',
        detail: 'src/main.rs',
      },
    })
  })
})
