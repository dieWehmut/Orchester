import { describe, expect, it } from 'vitest'

import {
  EVENT_TYPES,
  approvalId,
  isTerminal,
  isToolCall,
  parseEvent,
  parseEventJson,
  type Event,
  type EventType,
} from '../src/index'

/**
 * One sample per variant, keyed by tag.
 *
 * Typed as a `Record<EventType, …>` so the compiler refuses to build this file if
 * the union gains a member without a sample here. That is the half of the
 * drift check `satisfies` on `EVENT_TYPES` cannot do.
 */
const SAMPLES: Record<EventType, Event> = {
  session_started: { type: 'session_started', session_id: 'run-7' },
  turn_started: { type: 'turn_started' },
  message: { type: 'message', text: 'looking at the config loader' },
  reasoning: { type: 'reasoning', text: 'the loader resolves before the merge' },
  tool_call: { type: 'tool_call', name: 'read_file', status: 'completed', detail: 'path_bytes=24' },
  file_change: { type: 'file_change', path: 'kisten/netz/src/lib.rs', kind: 'add' },
  todo_list: {
    type: 'todo_list',
    items: [
      { text: 'bind loopback', completed: true },
      { text: 'mint the token', completed: false },
    ],
  },
  usage: {
    type: 'usage',
    input_tokens: 10,
    output_tokens: 20,
    cached_input_tokens: 5,
    reasoning_output_tokens: 3,
  },
  turn_completed: { type: 'turn_completed' },
  approval_required: {
    type: 'approval_required',
    approval_id: approvalId('apr-1'),
    action: 'write_file path_bytes=12 content_bytes=340',
    reason: 'writes outside the workspace',
  },
  result: { type: 'result', text: 'done' },
  stopped: { type: 'stopped', reason: 'awaiting_approval' },
  error: { type: 'error', message: 'boom' },
}

describe('the event union tracks the Rust protocol', () => {
  it('lists every tag exactly once', () => {
    expect([...EVENT_TYPES].sort()).toEqual(Object.keys(SAMPLES).sort())
  })

  it('covers thirteen variants', () => {
    // Pinned so that adding a Rust variant and forgetting the frontend shows up
    // as a failing count rather than as a frame the UI silently drops.
    expect(EVENT_TYPES).toHaveLength(13)
  })

  it('is exhaustive in a switch', () => {
    const describeEvent = (event: Event): string => {
      switch (event.type) {
        case 'session_started':
          return event.session_id
        case 'turn_started':
        case 'turn_completed':
          return event.type
        case 'message':
        case 'reasoning':
        case 'result':
          return event.text
        case 'tool_call':
          return `${event.name}:${event.status}`
        case 'file_change':
          return `${event.kind}:${event.path}`
        case 'todo_list':
          return String(event.items.length)
        case 'usage':
          return String(event.input_tokens + event.output_tokens)
        case 'approval_required':
          return event.approval_id
        case 'stopped':
          return event.reason
        case 'error':
          return event.message
        default: {
          const unreachable: never = event
          return unreachable
        }
      }
    }

    for (const sample of Object.values(SAMPLES)) {
      expect(describeEvent(sample)).toBeTypeOf('string')
    }
  })
})

describe('parseEvent', () => {
  it('round-trips every variant through JSON', () => {
    for (const [tag, sample] of Object.entries(SAMPLES)) {
      expect(parseEventJson(JSON.stringify(sample)), tag).toEqual(sample)
    }
  })

  it('keeps the usage fields beside the tag', () => {
    // The Rust side is a newtype variant, so the fields are flattened rather
    // than nested. Pinned in kisten/protokoll/tests/roundtrip.rs too.
    const decoded = parseEventJson('{"type":"usage","input_tokens":100,"output_tokens":200}')
    expect(decoded).toEqual({
      type: 'usage',
      input_tokens: 100,
      output_tokens: 200,
      cached_input_tokens: 0,
      reasoning_output_tokens: 0,
    })
  })

  it('omits an absent tool detail rather than setting it to undefined', () => {
    const decoded = parseEvent({ type: 'tool_call', name: 'list_files', status: 'in_progress' })
    expect(decoded).toEqual({ type: 'tool_call', name: 'list_files', status: 'in_progress' })
    expect(decoded && 'detail' in decoded).toBe(false)
  })

  it('rejects an unknown tag so a newer server cannot break the page', () => {
    expect(parseEvent({ type: 'quantum_entangled', text: 'hi' })).toBeNull()
  })

  it('rejects a known tag with a missing field', () => {
    expect(parseEvent({ type: 'message' })).toBeNull()
    expect(parseEvent({ type: 'tool_call', name: 'read_file' })).toBeNull()
    expect(parseEvent({ type: 'file_change', path: 'a.rs', kind: 'rename' })).toBeNull()
    expect(parseEvent({ type: 'stopped', reason: 'exploded' })).toBeNull()
  })

  it('rejects a non-numeric token count instead of reading it as zero', () => {
    // Under-reporting spend is worse than dropping the frame.
    expect(parseEvent({ type: 'usage', input_tokens: 'lots' })).toBeNull()
  })

  it('rejects non-objects and malformed JSON', () => {
    expect(parseEvent(null)).toBeNull()
    expect(parseEvent(['message'])).toBeNull()
    expect(parseEvent('message')).toBeNull()
    expect(parseEventJson('{"type":')).toBeNull()
  })
})

describe('narrowing helpers', () => {
  it('recognises a tool call', () => {
    expect(isToolCall(SAMPLES.tool_call)).toBe(true)
    expect(isToolCall(SAMPLES.message)).toBe(false)
  })

  it('treats stopped and error as terminal but not result', () => {
    expect(isTerminal(SAMPLES.stopped)).toBe(true)
    expect(isTerminal(SAMPLES.error)).toBe(true)
    // The runtime still owes a `stopped` after the final text.
    expect(isTerminal(SAMPLES.result)).toBe(false)
  })
})
