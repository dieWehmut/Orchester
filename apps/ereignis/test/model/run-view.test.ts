import { callId } from '@orchester/protokoll'
import { describe, expect, expectTypeOf, it } from 'vitest'

import {
  RUN_STATUSES,
  RUN_STATUS_LABEL_KEYS,
  TIMELINE_ITEM_TYPES,
  TIMELINE_LABEL_KEYS,
  createEmptyRunView,
  type TimelineItem,
  type ToolInvocationView,
} from '../../src/model/run-view'

describe('run view model', () => {
  it('starts with deterministic connection-independent state', () => {
    const view = createEmptyRunView()

    expect(view).toEqual({
      runId: null,
      title: null,
      status: 'idle',
      stop: null,
      turns: [],
      timeline: [],
      tools: [],
      approvals: [],
      usage: {
        input_tokens: 0,
        output_tokens: 0,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
      },
      validation: null,
      todos: [],
      fileChanges: [],
      errors: [],
      latestSequence: 0,
      bufferedSequences: [],
      gaps: [],
    })
    expect(Object.keys(RUN_STATUS_LABEL_KEYS)).toEqual(RUN_STATUSES)
    expect(RUN_STATUSES).toContain('running')
    expect(Object.keys(RUN_STATUS_LABEL_KEYS)).not.toContain('disconnected')
  })

  it('models message roles without transport concepts', () => {
    const user: TimelineItem = {
      type: 'message',
      key: 'message:user-1',
      sequence: 1,
      occurredAt: '2026-08-19T00:00:00.000Z',
      turnId: null,
      role: 'user',
      text: 'Inspect this workspace',
      final: true,
    }
    const assistant: TimelineItem = { ...user, key: 'message:assistant-1', role: 'assistant' }

    expect([user.role, assistant.role]).toEqual(['user', 'assistant'])
  })

  it('keeps timeline labels exhaustive and gap metadata synthetic', () => {
    const gap: TimelineItem = {
      type: 'gap',
      key: 'gap:run-fixture:2-3',
      sequence: 2,
      occurredAt: null,
      turnId: null,
      missingFrom: 2,
      missingTo: 3,
    }

    expect(Object.keys(TIMELINE_LABEL_KEYS)).toEqual(TIMELINE_ITEM_TYPES)
    expect(gap.occurredAt).toBeNull()
    expectTypeOf<Extract<TimelineItem, { type: 'gap' }>['occurredAt']>().toEqualTypeOf<null>()
  })

  it('models one tool invocation by call id with ordered lifecycle history', () => {
    const id = callId('call-read-1')
    const invocation: ToolInvocationView = {
      key: 'tool:call-read-1',
      callId: id,
      name: 'read_file',
      state: 'running',
      detail: 'src/main.ts',
      firstSequence: 3,
      lastSequence: 3,
      history: [
        {
          type: 'tool',
          key: 'tool:call-read-1:3',
          sequence: 3,
          occurredAt: '2026-08-19T00:00:02.000Z',
          turnId: null,
          callId: id,
          name: 'read_file',
          state: 'running',
          detail: 'src/main.ts',
        },
      ],
    }

    expect(invocation.history.map((item) => item.callId)).toEqual([id])
  })
})
