import { fixtureEnvelope } from '../fixtures'
import { callId, type UiEventEnvelope } from '../ui'

export function happyPathFixture(): UiEventEnvelope[] {
  const toolCallId = callId('call-happy-read')
  return [
    fixtureEnvelope(1, { type: 'run_started', title: 'Inspect workspace' }),
    fixtureEnvelope(2, { type: 'turn_started' }),
    fixtureEnvelope(3, { type: 'reasoning', text: 'Inspect the requested file' }),
    fixtureEnvelope(4, { type: 'message_delta', text: 'Checking', final: false }),
    fixtureEnvelope(5, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'read_file',
      state: 'running',
      detail: 'src/main.rs',
    }),
    fixtureEnvelope(6, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'read_file',
      state: 'succeeded',
      detail: '42 lines read',
    }),
    fixtureEnvelope(7, {
      type: 'validation',
      validation: { ok: true, summary: 'Focused checks passed' },
    }),
    fixtureEnvelope(8, {
      type: 'usage',
      input_tokens: 120,
      output_tokens: 48,
      cached_input_tokens: 32,
      reasoning_output_tokens: 8,
    }),
    fixtureEnvelope(9, { type: 'message', text: 'Workspace inspection completed.' }),
    fixtureEnvelope(10, { type: 'run_stopped', reason: 'succeeded' }),
  ]
}
