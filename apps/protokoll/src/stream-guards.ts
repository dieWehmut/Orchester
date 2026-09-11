import type { ResyncReason, RunStreamFrameDto } from './api'
import { runId } from './ui'
import { parseUiEventEnvelope } from './ui-guards'

type RecordValue = Record<string, unknown>
const RESYNC_REASONS: readonly ResyncReason[] = [
  'retention_exceeded',
  'sequence_gap',
  'schema_mismatch',
]

function isRecord(value: unknown): value is RecordValue {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function onlyKeys(value: RecordValue, keys: readonly string[]): boolean {
  return Object.keys(value).every((key) => keys.includes(key))
}

function positiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}

function nonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

export function parseRunStreamFrame(raw: unknown): RunStreamFrameDto | null {
  if (!isRecord(raw) || typeof raw.type !== 'string') return null

  if (raw.type === 'event') {
    if (!onlyKeys(raw, ['type', 'event'])) return null
    const event = parseUiEventEnvelope(raw.event)
    return event === null ? null : { type: 'event', event }
  }

  if (raw.type !== 'resync_required') return null
  if (
    !onlyKeys(raw, [
      'type',
      'run_id',
      'requested_after_sequence',
      'oldest_sequence',
      'latest_sequence',
      'reason',
    ]) ||
    !nonEmpty(raw.run_id) ||
    !nonNegativeInteger(raw.requested_after_sequence) ||
    !positiveInteger(raw.oldest_sequence) ||
    !nonNegativeInteger(raw.latest_sequence) ||
    typeof raw.reason !== 'string' ||
    !RESYNC_REASONS.includes(raw.reason as ResyncReason) ||
    raw.latest_sequence < raw.oldest_sequence - 1 ||
    raw.requested_after_sequence > raw.latest_sequence
  ) {
    return null
  }

  return {
    type: 'resync_required',
    run_id: runId(raw.run_id),
    requested_after_sequence: raw.requested_after_sequence,
    oldest_sequence: raw.oldest_sequence,
    latest_sequence: raw.latest_sequence,
    reason: raw.reason as ResyncReason,
  }
}

export function parseRunStreamFrameJson(text: string): RunStreamFrameDto | null {
  try {
    return parseRunStreamFrame(JSON.parse(text) as unknown)
  } catch {
    return null
  }
}
