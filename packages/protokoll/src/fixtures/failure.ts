import { fixtureEnvelope } from '../fixtures'
import { callId, type UiEventEnvelope } from '../ui'

export function failurePathFixture(): UiEventEnvelope[] {
  const toolCallId = callId('call-failure-check')
  return [
    fixtureEnvelope(1, { type: 'run_started', title: 'Run focused checks' }),
    fixtureEnvelope(2, { type: 'turn_started' }),
    fixtureEnvelope(3, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'run_checks',
      state: 'running',
      detail: 'protocol unit tests',
    }),
    fixtureEnvelope(4, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'run_checks',
      state: 'failed',
      detail: 'exit_code=1',
    }),
    fixtureEnvelope(5, {
      type: 'validation',
      validation: {
        ok: false,
        summary: 'Protocol checks failed',
        details: 'One focused test reported a mismatch',
      },
    }),
    fixtureEnvelope(6, {
      type: 'error',
      code: 'validation_failed',
      message: 'The run could not satisfy its validation gate.',
    }),
    fixtureEnvelope(7, { type: 'run_stopped', reason: 'failed' }),
  ]
}
