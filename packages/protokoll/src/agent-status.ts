/** Redaction-safe runtime status shared by the WebUI, website fixtures, and Tauri. */

export const AGENT_STATUS_SCHEMA_VERSION = 3 as const

export type AgentAvailabilityState =
  | 'available'
  | 'unavailable'
  | 'auth_required'
  | 'error'

export const AGENT_AVAILABILITY_STATES = [
  'available',
  'unavailable',
  'auth_required',
  'error',
] as const satisfies readonly AgentAvailabilityState[]

export type AgentActivityState =
  | 'offline'
  | 'idle'
  | 'starting'
  | 'running'
  | 'waiting_approval'
  | 'stopping'
  | 'error'
  | 'unknown'

export const AGENT_ACTIVITY_STATES = [
  'offline',
  'idle',
  'starting',
  'running',
  'waiting_approval',
  'stopping',
  'error',
  'unknown',
] as const satisfies readonly AgentActivityState[]

export type AgentWindowCountSource =
  | 'managed_sessions'
  | 'tauri_windows'
  | 'external_processes'

export const AGENT_WINDOW_COUNT_SOURCES = [
  'managed_sessions',
  'tauri_windows',
  'external_processes',
] as const satisfies readonly AgentWindowCountSource[]

export interface AgentProcessSourceStatusDto {
  environment: 'windows' | 'wsl' | 'local'
  status: 'available' | 'stale' | 'unavailable'
  count: number | null
  observed_at: string | null
}

export interface AgentRuntimeSummaryDto {
  agent_id: string
  provider: string
  display_name: string
  icon_key: string
  availability: AgentAvailabilityState
  activity: AgentActivityState
  installed: boolean
  configured: boolean
  authenticated: boolean
  active_windows: number
  active_sessions: number
  active_runs: number
  active_subagents: number
  window_count_source: AgentWindowCountSource
  process_sources: AgentProcessSourceStatusDto[] | null
  last_heartbeat_at: string | null
  last_error: string | null
  capabilities: string[]
  updated_at: string
}

export interface AgentFleetSnapshotDto {
  schema_version: 2 | typeof AGENT_STATUS_SCHEMA_VERSION
  sequence: number
  generated_at: string
  agents: AgentRuntimeSummaryDto[]
}

type UnknownRecord = Record<string, unknown>

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(record: UnknownRecord, allowed: readonly string[]): boolean {
  return Object.keys(record).every((key) => allowed.includes(key))
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null
}

function positiveInteger(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0 ? value : null
}

function count(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null
}

function boolean(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null
}

function member<T extends string>(value: unknown, allowed: readonly T[]): T | null {
  return typeof value === 'string' && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : null
}

function timestamp(value: unknown): string | null {
  const text = nonEmptyString(value)
  return text !== null && Number.isFinite(Date.parse(text)) ? text : null
}

function sourceTimestamp(value: unknown): string | null {
  const text = timestamp(value)
  if (text === null) return null
  const parts = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.exec(text)
  if (parts === null) return null
  const year = Number(parts[1]), month = Number(parts[2]), day = Number(parts[3])
  const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
  const days = [31, leapYear ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
  return month >= 1 && month <= 12 && day >= 1 && day <= days[month - 1]! &&
    Number(parts[4]) <= 23 && Number(parts[5]) <= 59 && Number(parts[6]) <= 59 ? text : null
}

function safeIconKey(value: unknown): string | null {
  const text = nonEmptyString(value)
  return text !== null && /^[a-z0-9][a-z0-9_-]*$/.test(text) ? text : null
}

function safeError(value: unknown): string | null | undefined {
  if (value === null) return null
  const text = nonEmptyString(value)
  if (text === null) return undefined
  // Paths, transcript locations, and credential-shaped values never cross the UI boundary.
  if (/(?:[A-Za-z]:\\|\\\\|\/(?:home|Users|private|tmp|var)\/)/i.test(text)) return undefined
  if (/(?:transcript(?:_path)?|api[_-]?key|token|secret|password)\s*[:=]/i.test(text)) {
    return undefined
  }
  return text
}

function capabilities(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null
  const result: string[] = []
  for (const entry of value) {
    const capability = nonEmptyString(entry)
    if (capability === null || capability.length > 80 || /[\\/]/.test(capability)) return null
    result.push(capability)
  }
  return result
}

function processSources(value: unknown): AgentProcessSourceStatusDto[] | null | undefined {
  if (value === null) return null
  if (!Array.isArray(value) || value.length === 0) return undefined
  const result: AgentProcessSourceStatusDto[] = []
  for (const entry of value) {
    if (!isRecord(entry) || !hasOnlyKeys(entry, ['environment', 'status', 'count', 'observed_at'])) return undefined
    const environment = member(entry.environment, ['windows', 'wsl', 'local'] as const)
    const status = member(entry.status, ['available', 'stale', 'unavailable'] as const)
    if (environment === null || status === null) return undefined
    if (status === 'unavailable') {
      if (entry.count !== null || entry.observed_at !== null) return undefined
      result.push({ environment, status, count: null, observed_at: null })
    } else {
      const observedCount = count(entry.count)
      const observedAt = sourceTimestamp(entry.observed_at)
      if (observedCount === null || observedAt === null) return undefined
      result.push({ environment, status, count: observedCount, observed_at: observedAt })
    }
  }
  return result
}

function parseAgentRuntimeSummary(raw: unknown, schemaVersion: 2 | 3): AgentRuntimeSummaryDto | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, [
      'agent_id',
      'provider',
      'display_name',
      'icon_key',
      'availability',
      'activity',
      'installed',
      'configured',
      'authenticated',
      'active_windows',
      'active_sessions',
      'active_runs',
      'active_subagents',
      'window_count_source',
      ...(schemaVersion === 3 ? ['process_sources'] : []),
      'last_heartbeat_at',
      'last_error',
      'capabilities',
      'updated_at',
    ])
  ) {
    return null
  }

  const agentId = nonEmptyString(raw.agent_id)
  const provider = nonEmptyString(raw.provider)
  const displayName = nonEmptyString(raw.display_name)
  const iconKey = safeIconKey(raw.icon_key)
  const availability = member(raw.availability, AGENT_AVAILABILITY_STATES)
  const activity = member(raw.activity, AGENT_ACTIVITY_STATES)
  if (schemaVersion === 2 && activity === 'unknown') return null
  const installed = boolean(raw.installed)
  const configured = boolean(raw.configured)
  const authenticated = boolean(raw.authenticated)
  const activeWindows = count(raw.active_windows)
  const activeSessions = count(raw.active_sessions)
  const activeRuns = count(raw.active_runs)
  const activeSubagents = count(raw.active_subagents)
  const windowCountSource = member(raw.window_count_source, AGENT_WINDOW_COUNT_SOURCES)
  const sources = schemaVersion === 2 ? null : processSources(raw.process_sources)
  const heartbeat = raw.last_heartbeat_at === null ? null : timestamp(raw.last_heartbeat_at)
  const lastError = safeError(raw.last_error)
  const capabilityList = capabilities(raw.capabilities)
  const updatedAt = timestamp(raw.updated_at)

  if (
    agentId === null ||
    provider === null ||
    displayName === null ||
    iconKey === null ||
    availability === null ||
    activity === null ||
    installed === null ||
    configured === null ||
    authenticated === null ||
    activeWindows === null ||
    activeSessions === null ||
    activeRuns === null ||
    activeSubagents === null ||
    windowCountSource === null ||
    sources === undefined ||
    heartbeat === undefined ||
    lastError === undefined ||
    capabilityList === null ||
    updatedAt === null
  ) {
    return null
  }

  if (windowCountSource === 'external_processes' && sources !== null) {
    const currentCount = sources.reduce((total, source) => total + (source.status === 'available' ? source.count ?? 0 : 0), 0)
    if (!Number.isSafeInteger(currentCount) || currentCount !== activeWindows) return null
  }

  return {
    agent_id: agentId,
    provider,
    display_name: displayName,
    icon_key: iconKey,
    availability,
    activity,
    installed,
    configured,
    authenticated,
    active_windows: activeWindows,
    active_sessions: activeSessions,
    active_runs: activeRuns,
    active_subagents: activeSubagents,
    window_count_source: windowCountSource,
    process_sources: sources,
    last_heartbeat_at: heartbeat,
    last_error: lastError,
    capabilities: capabilityList,
    updated_at: updatedAt,
  }
}

export function parseAgentFleetSnapshot(raw: unknown): AgentFleetSnapshotDto | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, ['schema_version', 'sequence', 'generated_at', 'agents']) ||
    (raw.schema_version !== 2 && raw.schema_version !== AGENT_STATUS_SCHEMA_VERSION)
  ) {
    return null
  }
  const sequence = positiveInteger(raw.sequence)
  const generatedAt = timestamp(raw.generated_at)
  if (sequence === null || generatedAt === null || !Array.isArray(raw.agents)) return null

  const agents: AgentRuntimeSummaryDto[] = []
  const ids = new Set<string>()
  for (const rawAgent of raw.agents) {
    const agent = parseAgentRuntimeSummary(rawAgent, raw.schema_version)
    if (agent === null || ids.has(agent.agent_id)) return null
    ids.add(agent.agent_id)
    agents.push(agent)
  }
  return { schema_version: raw.schema_version, sequence, generated_at: generatedAt, agents }
}

export function parseAgentFleetSnapshotJson(line: string): AgentFleetSnapshotDto | null {
  try {
    return parseAgentFleetSnapshot(JSON.parse(line) as unknown)
  } catch {
    return null
  }
}

export function isAgentFleetSnapshot(value: unknown): value is AgentFleetSnapshotDto {
  return isRecord(value) && value.schema_version === AGENT_STATUS_SCHEMA_VERSION && parseAgentFleetSnapshot(value) !== null
}
