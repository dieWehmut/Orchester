import {
  CHANGE_KINDS,
  STOP_REASONS,
  approvalId,
  type ChangeKind,
  type StopReason,
  type TodoItem,
} from './event'
import {
  UI_APPROVAL_DECISIONS,
  UI_SCHEMA_VERSION,
  UI_TOOL_STATES,
  callId,
  eventId,
  runId,
  turnId,
  type UiApprovalDecision,
  type UiApprovalRequest,
  type UiApprovalResolution,
  type UiEventEnvelope,
  type UiEventKind,
  type UiToolState,
  type UiValidation,
} from './ui'

type UnknownRecord = Record<string, unknown>

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(record: UnknownRecord, allowed: readonly string[]): boolean {
  return Object.keys(record).every((key) => allowed.includes(key))
}

function string(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

function nonEmptyString(value: unknown): string | null {
  const text = string(value)
  return text !== null && text.trim().length > 0 ? text : null
}

function optionalString(record: UnknownRecord, key: string): string | null | undefined {
  if (!(key in record)) return undefined
  return string(record[key])
}

function integer(value: unknown, positive = false): number | null {
  if (!Number.isSafeInteger(value) || typeof value !== 'number') return null
  if (positive ? value <= 0 : value < 0) return null
  return value
}

function member<T extends string>(value: unknown, allowed: readonly T[]): T | null {
  const text = string(value)
  return text !== null && (allowed as readonly string[]).includes(text) ? (text as T) : null
}

function parseTodoItems(value: unknown): TodoItem[] | null {
  if (!Array.isArray(value)) return null
  const items: TodoItem[] = []
  for (const item of value) {
    if (!isRecord(item) || !hasOnlyKeys(item, ['text', 'completed'])) return null
    const text = string(item.text)
    if (text === null || typeof item.completed !== 'boolean') return null
    items.push({ text, completed: item.completed })
  }
  return items
}

function parseApprovalRequest(value: unknown): UiApprovalRequest | null {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'approval_id',
      'run_id',
      'row_version',
      'risk',
      'action',
      'reason',
      'expires_at',
    ])
  ) {
    return null
  }
  const id = nonEmptyString(value.approval_id)
  const boundRunId = nonEmptyString(value.run_id)
  const rowVersion = integer(value.row_version, true)
  const risk = string(value.risk)
  const action = string(value.action)
  const reason = string(value.reason)
  const expiresAt = optionalString(value, 'expires_at')
  if (
    id === null ||
    boundRunId === null ||
    rowVersion === null ||
    risk === null ||
    action === null ||
    reason === null ||
    expiresAt === null
  ) {
    return null
  }
  const approval: UiApprovalRequest = {
    approval_id: approvalId(id),
    run_id: runId(boundRunId),
    row_version: rowVersion,
    risk,
    action,
    reason,
  }
  return expiresAt === undefined ? approval : { ...approval, expires_at: expiresAt }
}

function parseApprovalResolution(value: unknown): UiApprovalResolution | null {
  if (!isRecord(value) || !hasOnlyKeys(value, ['approval_id', 'row_version', 'decision'])) {
    return null
  }
  const id = nonEmptyString(value.approval_id)
  const rowVersion = integer(value.row_version, true)
  const decision: UiApprovalDecision | null = member(value.decision, UI_APPROVAL_DECISIONS)
  return id === null || rowVersion === null || decision === null
    ? null
    : { approval_id: approvalId(id), row_version: rowVersion, decision }
}

function parseValidation(value: unknown): UiValidation | null {
  if (!isRecord(value) || !hasOnlyKeys(value, ['ok', 'summary', 'details'])) return null
  const summary = string(value.summary)
  const details = optionalString(value, 'details')
  if (typeof value.ok !== 'boolean' || summary === null || details === null) return null
  const validation: UiValidation = { ok: value.ok, summary }
  return details === undefined ? validation : { ...validation, details }
}

function parseEventKind(raw: unknown): UiEventKind | null {
  if (!isRecord(raw)) return null
  const tag = string(raw.type)
  if (tag === null) return null

  switch (tag) {
    case 'run_started': {
      if (!hasOnlyKeys(raw, ['type', 'title'])) return null
      const title = optionalString(raw, 'title')
      if (title === null) return null
      return title === undefined ? { type: tag } : { type: tag, title }
    }
    case 'turn_started':
      return hasOnlyKeys(raw, ['type']) ? { type: tag } : null
    case 'message':
    case 'reasoning': {
      if (!hasOnlyKeys(raw, ['type', 'text'])) return null
      const text = string(raw.text)
      return text === null ? null : { type: tag, text }
    }
    case 'message_delta': {
      if (!hasOnlyKeys(raw, ['type', 'text', 'final'])) return null
      const text = string(raw.text)
      const final = raw.final === undefined ? false : raw.final
      return text === null || typeof final !== 'boolean' ? null : { type: tag, text, final }
    }
    case 'tool_call': {
      if (!hasOnlyKeys(raw, ['type', 'call_id', 'name', 'state', 'detail'])) return null
      const id = nonEmptyString(raw.call_id)
      const name = string(raw.name)
      const state: UiToolState | null = member(raw.state, UI_TOOL_STATES)
      const detail = optionalString(raw, 'detail')
      if (id === null || name === null || state === null || detail === null) return null
      const event: UiEventKind = { type: tag, call_id: callId(id), name, state }
      return detail === undefined ? event : { ...event, detail }
    }
    case 'file_change': {
      if (!hasOnlyKeys(raw, ['type', 'path', 'kind'])) return null
      const path = string(raw.path)
      const kind: ChangeKind | null = member(raw.kind, CHANGE_KINDS)
      return path === null || kind === null ? null : { type: tag, path, kind }
    }
    case 'todo_list': {
      if (!hasOnlyKeys(raw, ['type', 'items'])) return null
      const items = parseTodoItems(raw.items)
      return items === null ? null : { type: tag, items }
    }
    case 'usage': {
      if (
        !hasOnlyKeys(raw, [
          'type',
          'input_tokens',
          'output_tokens',
          'cached_input_tokens',
          'reasoning_output_tokens',
        ])
      ) {
        return null
      }
      const input = integer(raw.input_tokens)
      const output = integer(raw.output_tokens)
      const cached = integer(raw.cached_input_tokens)
      const reasoning = integer(raw.reasoning_output_tokens)
      return input === null || output === null || cached === null || reasoning === null
        ? null
        : {
            type: tag,
            input_tokens: input,
            output_tokens: output,
            cached_input_tokens: cached,
            reasoning_output_tokens: reasoning,
          }
    }
    case 'approval_requested': {
      if (!hasOnlyKeys(raw, ['type', 'approval'])) return null
      const approval = parseApprovalRequest(raw.approval)
      return approval === null ? null : { type: tag, approval }
    }
    case 'approval_resolved': {
      if (!hasOnlyKeys(raw, ['type', 'resolution'])) return null
      const resolution = parseApprovalResolution(raw.resolution)
      return resolution === null ? null : { type: tag, resolution }
    }
    case 'validation': {
      if (!hasOnlyKeys(raw, ['type', 'validation'])) return null
      const validation = parseValidation(raw.validation)
      return validation === null ? null : { type: tag, validation }
    }
    case 'run_stopped': {
      if (!hasOnlyKeys(raw, ['type', 'reason'])) return null
      const reason: StopReason | null = member(raw.reason, STOP_REASONS)
      return reason === null ? null : { type: tag, reason }
    }
    case 'error': {
      if (!hasOnlyKeys(raw, ['type', 'code', 'message'])) return null
      const code = string(raw.code)
      const message = string(raw.message)
      return code === null || message === null ? null : { type: tag, code, message }
    }
    default:
      return null
  }
}

/** Validate an untrusted browser frame against the Rust UI wire contract. */
export function parseUiEventEnvelope(raw: unknown): UiEventEnvelope | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, [
      'schema_version',
      'event_id',
      'run_id',
      'turn_id',
      'call_id',
      'sequence',
      'occurred_at',
      'kind',
    ]) ||
    raw.schema_version !== UI_SCHEMA_VERSION
  ) {
    return null
  }

  const rawEventId = nonEmptyString(raw.event_id)
  const rawRunId = nonEmptyString(raw.run_id)
  const rawTurnId = optionalString(raw, 'turn_id')
  const rawCallId = optionalString(raw, 'call_id')
  const sequence = integer(raw.sequence, true)
  const occurredAt = nonEmptyString(raw.occurred_at)
  const kind = parseEventKind(raw.kind)
  if (
    rawEventId === null ||
    rawRunId === null ||
    rawTurnId === null ||
    rawCallId === null ||
    sequence === null ||
    occurredAt === null ||
    kind === null
  ) {
    return null
  }
  if (kind.type === 'tool_call' && rawCallId !== kind.call_id) return null
  if (kind.type === 'approval_requested' && kind.approval.run_id !== rawRunId) return null

  const envelope: UiEventEnvelope = {
    schema_version: UI_SCHEMA_VERSION,
    event_id: eventId(rawEventId),
    run_id: runId(rawRunId),
    sequence,
    occurred_at: occurredAt,
    kind,
  }
  const withTurn = rawTurnId === undefined ? envelope : { ...envelope, turn_id: turnId(rawTurnId) }
  return rawCallId === undefined ? withTurn : { ...withTurn, call_id: callId(rawCallId) }
}

export function parseUiEventEnvelopeJson(line: string): UiEventEnvelope | null {
  try {
    return parseUiEventEnvelope(JSON.parse(line) as unknown)
  } catch {
    return null
  }
}
