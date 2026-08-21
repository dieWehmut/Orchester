/** Redaction-safe runtime status shared by the WebUI, website fixtures, and Tauri. */

export const AGENT_STATUS_SCHEMA_VERSION = 2 as const

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

export const AGENT_ACTIVITY_STATES = [
  'offline',
  'idle',
  'starting',
  'running',
  'waiting_approval',
  'stopping',
  'error',
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
  last_heartbeat_at: string | null
  last_error: string | null
  capabilities: string[]
  updated_at: string
}

export interface AgentFleetSnapshotDto {
  schema_version: typeof AGENT_STATUS_SCHEMA_VERSION
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

function parseAgentRuntimeSummary(raw: unknown): AgentRuntimeSummaryDto | null {
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
  const installed = boolean(raw.installed)
  const configured = boolean(raw.configured)
  const authenticated = boolean(raw.authenticated)
  const activeWindows = count(raw.active_windows)
  const activeSessions = count(raw.active_sessions)
  const activeRuns = count(raw.active_runs)
  const activeSubagents = count(raw.active_subagents)
  const windowCountSource = member(raw.window_count_source, AGENT_WINDOW_COUNT_SOURCES)
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
    heartbeat === undefined ||
    lastError === undefined ||
    capabilityList === null ||
    updatedAt === null
  ) {
    return null
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
    raw.schema_version !== AGENT_STATUS_SCHEMA_VERSION
  ) {
    return null
  }
  const sequence = positiveInteger(raw.sequence)
  const generatedAt = timestamp(raw.generated_at)
  if (sequence === null || generatedAt === null || !Array.isArray(raw.agents)) return null

  const agents: AgentRuntimeSummaryDto[] = []
  const ids = new Set<string>()
  for (const rawAgent of raw.agents) {
    const agent = parseAgentRuntimeSummary(rawAgent)
    if (agent === null || ids.has(agent.agent_id)) return null
    ids.add(agent.agent_id)
    agents.push(agent)
  }
  return { schema_version: AGENT_STATUS_SCHEMA_VERSION, sequence, generated_at: generatedAt, agents }
}

export function parseAgentFleetSnapshotJson(line: string): AgentFleetSnapshotDto | null {
  try {
    return parseAgentFleetSnapshot(JSON.parse(line) as unknown)
  } catch {
    return null
  }
}

export function isAgentFleetSnapshot(value: unknown): value is AgentFleetSnapshotDto {
  return parseAgentFleetSnapshot(value) !== null
}
