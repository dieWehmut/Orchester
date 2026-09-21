import {
  approvalId,
  callId,
  fixtureEnvelope,
  runId,
  turnId,
  type RunSnapshotDto,
} from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'
import { projectRunEvents, projectRunSnapshot } from '../../src/model/project-run'

describe('conversation projection', () => {
  it('shows completed messages and assembles streaming text without replay duplicates', () => {
    const delta = fixtureEnvelope(3, { type: 'message_delta', text: 'Hello ', final: false })
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'turn_started' }),
      fixtureEnvelope(2, { type: 'message', text: 'Starting the task.' }),
      delta,
      delta,
      fixtureEnvelope(4, { type: 'message_delta', text: 'world', final: true }),
      fixtureEnvelope(5, { type: 'message_delta', text: 'Next response', final: true }),
    ])
    expect(view.timeline).toMatchObject([
      { type: 'message', role: 'assistant', text: 'Starting the task.', final: true },
      { type: 'message', role: 'assistant', text: 'Hello world', final: true },
      { type: 'message', role: 'assistant', text: 'Next response', final: true },
    ])
    expect(new Set(view.timeline.map((item) => item.key)).size).toBe(3)
    expect(view.turns).toMatchObject([{ id: 'turn-fixture', items: view.timeline }])
  })

  it('preserves visible text before a gap while withholding later text', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'message', text: 'Already received' }),
      fixtureEnvelope(3, { type: 'message', text: 'Wait for replay' }),
    ])
    expect(view.timeline).toMatchObject([
      { type: 'message', text: 'Already received' },
      { type: 'gap', missingFrom: 2, missingTo: 2 },
    ])
  })

  it('keeps streaming responses in their own turns', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'message_delta', text: 'First', final: false }),
      { ...fixtureEnvelope(2, { type: 'turn_started' }), turn_id: turnId('second') },
      { ...fixtureEnvelope(3, { type: 'message_delta', text: 'Second', final: true }), turn_id: turnId('second') },
    ])
    expect(view.timeline.map((item) => item.type === 'message' && item.text)).toEqual(['First', 'Second'])
    expect(view.turns.map((turn) => turn.items.length)).toEqual([1, 1])
  })

  it('updates one tool row while keeping its event history', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'tool_call', call_id: callId('read'), name: 'read_file', state: 'running' }),
      fixtureEnvelope(2, { type: 'tool_call', call_id: callId('read'), name: 'read_file', state: 'succeeded', detail: 'Read README.md' }),
    ])
    expect(view.timeline).toMatchObject([{ type: 'tool', state: 'succeeded', detail: 'Read README.md' }])
    expect(view.tools).toMatchObject([{
      callId: 'read', state: 'succeeded', firstSequence: 1, lastSequence: 2,
      history: [{ state: 'running' }, { state: 'succeeded' }],
    }])
  })

  it('projects approval decisions with their original request details', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'approval_requested', approval: {
        approval_id: approvalId('approval-1'), run_id: runId('run-fixture'), row_version: 1,
        risk: 'write', action: 'Update README.md', reason: 'Write project file',
      } }),
      fixtureEnvelope(2, { type: 'approval_resolved', resolution: {
        approval_id: approvalId('approval-1'), row_version: 2, decision: 'approved',
      } }),
    ])
    expect(view.timeline).toMatchObject([{
      type: 'approval', state: 'approved', request: { action: 'Update README.md' }, decision: 'approved',
    }])
    expect(view.approvals).toMatchObject([{
      approvalId: 'approval-1', rowVersion: 2, action: 'Update README.md',
      state: 'approved', requestedSequence: 1, resolvedSequence: 2,
    }])
  })

  it('restores pending approvals when their request event has left the snapshot window', () => {
    const snapshot: RunSnapshotDto = {
      run_id: runId('run-fixture'), state: 'awaiting_approval', events: [],
      pending_approvals: [{ approval_id: approvalId('approval-older'), run_id: runId('run-fixture'),
        row_version: 3, risk: 'execute', action: 'Run tests', reason: 'Command needs approval' }],
      oldest_sequence: 0, latest_sequence: 0, next_sequence: 1,
      updated_at: '2026-09-17T00:00:00Z',
    }
    expect(projectRunSnapshot(snapshot).approvals).toMatchObject([{
      approvalId: 'approval-older', rowVersion: 3, state: 'pending', requestedSequence: null,
    }])
  })

  it('projects reasoning, changes, todos, validation, usage and errors', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'reasoning', text: 'Inspecting the project' }),
      fixtureEnvelope(2, { type: 'file_change', path: 'a.ts', kind: 'add' }),
      fixtureEnvelope(3, { type: 'todo_list', items: [{ text: 'Run tests', completed: false }] }),
      fixtureEnvelope(4, { type: 'validation', validation: { ok: true, summary: 'Tests pass' } }),
      fixtureEnvelope(5, { type: 'usage', input_tokens: 10, output_tokens: 5, cached_input_tokens: 2, reasoning_output_tokens: 1 }),
      fixtureEnvelope(6, { type: 'error', code: 'test_error', message: 'Visible failure' }),
    ])
    expect(view.timeline.map((item) => item.type)).toEqual(['reasoning', 'file_change', 'todo_list', 'validation', 'error'])
    expect(view.validation).toEqual({ ok: true, summary: 'Tests pass' })
    expect(view.todos).toEqual([{ text: 'Run tests', completed: false }])
    expect(view.usage).toEqual({ input_tokens: 10, output_tokens: 5, cached_input_tokens: 2, reasoning_output_tokens: 1 })
    expect(view.errors).toMatchObject([{ code: 'test_error', message: 'Visible failure' }])
  })
})
