/**
 * Validation at the wire boundary.
 *
 * A frontend can be older than the server it is talking to — a browser tab left
 * open across an upgrade is the normal case, not an edge case — so an unknown tag
 * has to be survivable. {@link parseEvent} returns `null` for anything it does not
 * recognise instead of throwing, and callers skip those frames. The alternative,
 * trusting the JSON and letting a missing field surface as `undefined` deep in a
 * template, turns a protocol addition into a blank screen.
 */

import {
  CHANGE_KINDS,
  STOP_REASONS,
  TOOL_STATUSES,
  approvalId,
  type ChangeKind,
  type Event,
  type StopReason,
  type ToolCallEvent,
  type ToolStatus,
  type TodoItem,
} from './event'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function str(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

/**
 * Read a token count.
 *
 * Absent means zero, matching `#[serde(default)]` on the Rust side. A present but
 * non-numeric value is a protocol violation and rejects the whole event, because
 * silently reading it as zero would under-report spend.
 */
function tokens(value: unknown): number | null {
  if (value === undefined) return 0
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

function member<T extends string>(value: unknown, allowed: readonly T[]): T | null {
  const text = str(value)
  return text !== null && (allowed as readonly string[]).includes(text) ? (text as T) : null
}

function todoItems(value: unknown): TodoItem[] | null {
  if (!Array.isArray(value)) return null
  const items: TodoItem[] = []
  for (const entry of value) {
    if (!isRecord(entry)) return null
    const text = str(entry.text)
    if (text === null || typeof entry.completed !== 'boolean') return null
    items.push({ text, completed: entry.completed })
  }
  return items
}

/** Narrow one decoded frame into an {@link Event}, or `null` if it is not one. */
export function parseEvent(raw: unknown): Event | null {
  if (!isRecord(raw)) return null
  const tag = str(raw.type)
  if (tag === null) return null

  switch (tag) {
    case 'session_started': {
      const sessionId = str(raw.session_id)
      return sessionId === null ? null : { type: 'session_started', session_id: sessionId }
    }
    case 'turn_started':
      return { type: 'turn_started' }
    case 'turn_completed':
      return { type: 'turn_completed' }
    case 'message':
    case 'reasoning':
    case 'result': {
      const text = str(raw.text)
      return text === null ? null : { type: tag, text }
    }
    case 'tool_call': {
      const name = str(raw.name)
      const status: ToolStatus | null = member(raw.status, TOOL_STATUSES)
      if (name === null || status === null) return null
      const detail = str(raw.detail)
      // Built in two steps because `exactOptionalPropertyTypes` distinguishes an
      // absent `detail` from one explicitly set to `undefined`, and the Rust side
      // omits the field rather than nulling it.
      const event: ToolCallEvent = { type: 'tool_call', name, status }
      return detail === null ? event : { ...event, detail }
    }
    case 'file_change': {
      const path = str(raw.path)
      const kind: ChangeKind | null = member(raw.kind, CHANGE_KINDS)
      return path === null || kind === null ? null : { type: 'file_change', path, kind }
    }
    case 'todo_list': {
      const items = todoItems(raw.items)
      return items === null ? null : { type: 'todo_list', items }
    }
    case 'usage': {
      const input = tokens(raw.input_tokens)
      const output = tokens(raw.output_tokens)
      const cached = tokens(raw.cached_input_tokens)
      const reasoning = tokens(raw.reasoning_output_tokens)
      if (input === null || output === null || cached === null || reasoning === null) return null
      return {
        type: 'usage',
        input_tokens: input,
        output_tokens: output,
        cached_input_tokens: cached,
        reasoning_output_tokens: reasoning,
      }
    }
    case 'approval_required': {
      const id = str(raw.approval_id)
      const action = str(raw.action)
      const reason = str(raw.reason)
      if (id === null || action === null || reason === null) return null
      return { type: 'approval_required', approval_id: approvalId(id), action, reason }
    }
    case 'stopped': {
      const reason: StopReason | null = member(raw.reason, STOP_REASONS)
      return reason === null ? null : { type: 'stopped', reason }
    }
    case 'error': {
      const message = str(raw.message)
      return message === null ? null : { type: 'error', message }
    }
    default:
      return null
  }
}

/** Decode one JSONL line. Malformed JSON is treated the same as an unknown tag. */
export function parseEventJson(line: string): Event | null {
  try {
    return parseEvent(JSON.parse(line) as unknown)
  } catch {
    return null
  }
}

export function isToolCall(event: Event): event is ToolCallEvent {
  return event.type === 'tool_call'
}

/**
 * Whether this event ends the stream.
 *
 * `stopped` covers the governed outcomes — including waiting for an approval,
 * which ends the *stream* without ending the *task* — and `error` covers a fatal
 * one. A `result` is not terminal: the runtime still owes a `stopped` after it.
 */
export function isTerminal(event: Event): boolean {
  return event.type === 'stopped' || event.type === 'error'
}
