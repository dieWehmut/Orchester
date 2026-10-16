import type { HttpClient } from './http'

export const activityProviders = ['codex', 'claude', 'opencode'] as const
export type ExternalActivityProvider = typeof activityProviders[number]
export type ActivityProvider = ExternalActivityProvider | 'orchester'
const supportedProviders: readonly string[] = [...activityProviders, 'orchester']
export const activityNames: Record<ExternalActivityProvider, string> = { codex: 'Codex', claude: 'Claude Code', opencode: 'OpenCode' }
export interface ActivitySummary {
  id: string
  provider: ActivityProvider
  source: 'external' | 'managed'
  title: string
  project: string
  project_id?: string
  pinned?: boolean
  updated_at: number
  state: 'active' | 'completed' | 'interrupted' | 'unknown'
  preview: string
  truncated: boolean
}
export interface ActivityEntry {
  id: string
  kind: 'user' | 'assistant' | 'tool' | 'result' | 'status'
  text: string
  tool: string | null
  timestamp?: number
  phase?: 'commentary' | 'final'
  turn_id?: string
  call_id?: string
}
export interface ActivityTurn { id: string; started_at?: number; completed_at?: number }
export interface ActivityDetail extends ActivitySummary { schema_version: 1; entries: ActivityEntry[]; turns?: ActivityTurn[] }
export interface ActivitySource { provider: ActivityProvider; status: 'available' | 'not_found' | 'unavailable' | 'partial' }
export interface ActivityList { schema_version: 1; observed_at: number; sources: ActivitySource[]; sessions: ActivitySummary[] }

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value) }
function optionalTime(value: unknown): boolean { return value === undefined || (typeof value === 'number' && Number.isSafeInteger(value) && Math.abs(value) <= 8.64e15) }
function summary(value: unknown): value is ActivitySummary {
  return record(value) && ['id', 'title', 'project', 'preview'].every(key => typeof value[key] === 'string')
    && supportedProviders.includes(String(value.provider)) && ['external', 'managed'].includes(String(value.source))
    && (value.project_id === undefined || typeof value.project_id === 'string') && (value.pinned === undefined || typeof value.pinned === 'boolean')
    && ['active', 'completed', 'interrupted', 'unknown'].includes(String(value.state))
    && typeof value.updated_at === 'number' && Number.isSafeInteger(value.updated_at) && Math.abs(value.updated_at) <= 8.64e15 && typeof value.truncated === 'boolean'
}

export function createAgentActivityApi(http: HttpClient) {
  return {
    async list(provider: ActivityProvider, signal: AbortSignal): Promise<ActivityList> {
      const data = await http.get<unknown>(`/agents/activity?provider=${provider}`, { signal })
      if (!record(data) || data.schema_version !== 1 || typeof data.observed_at !== 'number' || !Number.isFinite(data.observed_at) || !Array.isArray(data.sessions) || !data.sessions.every(summary)
        || !Array.isArray(data.sources) || !data.sources.every(source => record(source) && supportedProviders.includes(String(source.provider)) && ['available', 'not_found', 'unavailable', 'partial'].includes(String(source.status)))) throw new Error('Invalid activity response')
      return data as unknown as ActivityList
    },
    async detail(id: string, signal: AbortSignal): Promise<ActivityDetail> {
      const data = await http.get<unknown>(`/agents/activity/${encodeURIComponent(id)}`, { signal })
      if (!summary(data) || !record(data) || data.schema_version !== 1 || !Array.isArray(data.entries)
        || !data.entries.every(entry => record(entry) && typeof entry.id === 'string' && typeof entry.text === 'string' && ['user', 'assistant', 'tool', 'result', 'status'].includes(String(entry.kind)) && (entry.tool === null || typeof entry.tool === 'string')
          && optionalTime(entry.timestamp) && (entry.phase === undefined || entry.phase === 'commentary' || entry.phase === 'final') && (entry.turn_id === undefined || typeof entry.turn_id === 'string') && (entry.call_id === undefined || typeof entry.call_id === 'string'))
        || (data.turns !== undefined && (!Array.isArray(data.turns) || data.turns.length > 100 || !data.turns.every(turn => record(turn) && typeof turn.id === 'string' && optionalTime(turn.started_at) && optionalTime(turn.completed_at))))) throw new Error('Invalid activity detail')
      return data as unknown as ActivityDetail
    },
  }
}
