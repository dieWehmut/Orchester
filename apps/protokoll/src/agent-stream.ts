import {
  AGENT_STATUS_SCHEMA_VERSION,
  parseAgentFleetSnapshot,
  type AgentFleetSnapshotDto,
} from './agent-status'

export type AgentFleetStreamFrameDto =
  | { type: 'snapshot'; snapshot: AgentFleetSnapshotDto }
  | { type: 'heartbeat'; sequence: number; sent_at: string }

type RecordValue = Record<string, unknown>

function isRecord(value: unknown): value is RecordValue {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function onlyKeys(value: RecordValue, keys: readonly string[]): boolean {
  return Object.keys(value).every((key) => keys.includes(key))
}

function positiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}

function timestamp(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && Number.isFinite(Date.parse(value))
}

export function parseAgentFleetStreamFrame(raw: unknown): AgentFleetStreamFrameDto | null {
  if (!isRecord(raw) || typeof raw.type !== 'string') return null

  if (raw.type === 'snapshot') {
    if (!onlyKeys(raw, ['type', 'snapshot'])) return null
    const snapshot = parseAgentFleetSnapshot(raw.snapshot)
    return snapshot === null || snapshot.schema_version !== AGENT_STATUS_SCHEMA_VERSION
      ? null
      : { type: 'snapshot', snapshot }
  }

  if (
    raw.type !== 'heartbeat' ||
    !onlyKeys(raw, ['type', 'sequence', 'sent_at']) ||
    !positiveInteger(raw.sequence) ||
    !timestamp(raw.sent_at)
  ) {
    return null
  }

  return {
    type: 'heartbeat',
    sequence: raw.sequence,
    sent_at: raw.sent_at,
  }
}

export function parseAgentFleetStreamFrameJson(text: string): AgentFleetStreamFrameDto | null {
  try {
    return parseAgentFleetStreamFrame(JSON.parse(text) as unknown)
  } catch {
    return null
  }
}
