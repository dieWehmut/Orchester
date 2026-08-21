import { describe, expect, it } from 'vitest'

import {
  callId,
  runId,
  toolInvocationKey,
  turnId,
  type ToolInvocationDto,
  type ToolInvocationResultDto,
} from '../src/index'

const first: ToolInvocationDto = {
  call_id: callId('call-1'),
  run_id: runId('run-1'),
  turn_id: turnId('turn-1'),
  name: 'read_file',
  state: 'running',
  detail: 'src/a.rs',
  started_at: '2026-08-19T00:00:00Z',
}

const second: ToolInvocationDto = {
  call_id: callId('call-2'),
  run_id: runId('run-1'),
  turn_id: turnId('turn-1'),
  name: 'read_file',
  state: 'queued',
  detail: 'src/b.rs',
}

describe('tool invocation DTOs', () => {
  it('keys concurrent tools only by call id', () => {
    const byCallId = new Map([first, second].map((tool) => [toolInvocationKey(tool), tool]))

    expect(byCallId).toHaveLength(2)
    expect(byCallId.get(callId('call-1'))?.detail).toBe('src/a.rs')
    expect(byCallId.get(callId('call-2'))?.detail).toBe('src/b.rs')
  })

  it('binds terminal results back to the same call id', () => {
    const result: ToolInvocationResultDto = {
      call_id: first.call_id,
      state: 'succeeded',
      detail: 'file contents available',
      completed_at: '2026-08-19T00:00:01Z',
    }

    expect(result.call_id).toBe(first.call_id)
    expect(result.state).toBe('succeeded')
  })
})
