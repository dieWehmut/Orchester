import { fixtureEnvelope } from '../fixtures'
import type { RunReplayRequestDto, RunReplayResponseDto } from '../api'
import { callId, type UiEventEnvelope } from '../ui'

export interface ReconnectPathFixture {
  before_disconnect: UiEventEnvelope[]
  replay_request: RunReplayRequestDto
  replay_response: RunReplayResponseDto
}

export function reconnectPathFixture(): ReconnectPathFixture {
  const toolCallId = callId('call-reconnect-read')
  const events = [
    fixtureEnvelope(1, { type: 'run_started', title: 'Reconnect demo' }),
    fixtureEnvelope(2, { type: 'turn_started' }),
    fixtureEnvelope(3, { type: 'message', text: 'Connection is healthy.' }),
    fixtureEnvelope(4, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'read_file',
      state: 'running',
      detail: 'src/lib.rs',
    }),
    fixtureEnvelope(5, {
      type: 'tool_call',
      call_id: toolCallId,
      name: 'read_file',
      state: 'succeeded',
      detail: 'file read',
    }),
    fixtureEnvelope(6, { type: 'message_delta', text: 'Recovered', final: false }),
    fixtureEnvelope(7, { type: 'message_delta', text: ' stream', final: true }),
    fixtureEnvelope(8, {
      type: 'usage',
      input_tokens: 20,
      output_tokens: 8,
      cached_input_tokens: 0,
      reasoning_output_tokens: 1,
    }),
    fixtureEnvelope(9, { type: 'message', text: 'Replay completed.' }),
    fixtureEnvelope(10, { type: 'run_stopped', reason: 'succeeded' }),
  ]
  return {
    before_disconnect: events.slice(0, 5),
    replay_request: { after_sequence: 3, limit: 100 },
    replay_response: {
      run_id: events[0]!.run_id,
      events: events.slice(3),
      first_sequence: 4,
      last_sequence: 10,
      has_more: false,
    },
  }
}
