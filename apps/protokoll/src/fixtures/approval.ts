import { fixtureEnvelope } from '../fixtures'
import { approvalId } from '../event'
import { callId, runId, type UiEventEnvelope } from '../ui'

export function approvalPathFixture(): UiEventEnvelope[] {
  const toolCallId = callId('call-approval-write')
  const requestId = approvalId('approval-fixture')
  return [
    fixtureEnvelope(1, { type: 'run_started', title: 'Update source file' }),
    fixtureEnvelope(2, { type: 'turn_started' }),
    fixtureEnvelope(3, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'write_file',
      state: 'queued',
      detail: 'src/main.rs',
    }),
    fixtureEnvelope(4, {
      type: 'approval_requested',
      approval: {
        approval_id: requestId,
        run_id: runId('run-fixture'),
        row_version: 1,
        risk: 'workspace_write',
        action: 'write_file path=src/main.rs',
        reason: 'The action modifies source code',
        expires_at: '2026-08-19T00:15:00Z',
      },
    }),
    fixtureEnvelope(5, { type: 'run_stopped', reason: 'awaiting_approval' }),
    fixtureEnvelope(6, {
      type: 'approval_resolved',
      resolution: {
        approval_id: requestId,
        row_version: 2,
        decision: 'approved',
      },
    }),
    fixtureEnvelope(7, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'write_file',
      state: 'running',
      detail: 'src/main.rs',
    }),
    fixtureEnvelope(8, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'write_file',
      state: 'succeeded',
      detail: 'source file updated',
    }),
    fixtureEnvelope(9, { type: 'message', text: 'Approved change completed.' }),
    fixtureEnvelope(10, { type: 'run_stopped', reason: 'succeeded' }),
  ]
}
