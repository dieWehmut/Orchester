import type { HttpClient } from './http'

export const activityProviders = ['codex', 'claude', 'opencode', 'orchester'] as const
export type ActivityProvider = typeof activityProviders[number]
export interface ActivitySummary {
  id: string
  provider: ActivityProvider
  source: 'external' | 'managed'
  title: string
  project: string
  updated_at: number
  state: 'active' | 'completed' | 'interrupted' | 'unknown'
  preview: string
  truncated: boolean
}
export interface ActivityEntry { id: string; kind: 'user' | 'assistant' | 'tool' | 'result' | 'status'; text: string; tool: string | null }
export interface ActivityDetail extends ActivitySummary { schema_version: 1; entries: ActivityEntry[] }
export interface ActivitySource { provider: ActivityProvider; status: 'available' | 'not_found' | 'unavailable' | 'partial' }
export interface ActivityList { schema_version: 1; observed_at: number; sources: ActivitySource[]; sessions: ActivitySummary[] }

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value) }
function summary(value: unknown): value is ActivitySummary {
  return record(value) && ['id', 'title', 'project', 'preview'].every(key => typeof value[key] === 'string')
    && activityProviders.includes(value.provider as ActivityProvider) && ['external', 'managed'].includes(String(value.source))
    && ['active', 'completed', 'interrupted', 'unknown'].includes(String(value.state))
    && typeof value.updated_at === 'number' && Number.isSafeInteger(value.updated_at) && Math.abs(value.updated_at) <= 8.64e15 && typeof value.truncated === 'boolean'
}

export function createAgentActivityApi(http: HttpClient) {
  return {
    async list(provider: ActivityProvider, signal: AbortSignal): Promise<ActivityList> {
      const data = await http.get<unknown>(`/agents/activity?provider=${provider}`, { signal })
      if (!record(data) || data.schema_version !== 1 || typeof data.observed_at !== 'number' || !Number.isFinite(data.observed_at) || !Array.isArray(data.sessions) || !data.sessions.every(summary)
        || !Array.isArray(data.sources) || !data.sources.every(source => record(source) && activityProviders.includes(source.provider as ActivityProvider) && ['available', 'not_found', 'unavailable', 'partial'].includes(String(source.status)))) throw new Error('Invalid activity response')
      return data as unknown as ActivityList
    },
    async detail(id: string, signal: AbortSignal): Promise<ActivityDetail> {
      const data = await http.get<unknown>(`/agents/activity/${encodeURIComponent(id)}`, { signal })
      if (!summary(data) || !record(data) || data.schema_version !== 1 || !Array.isArray(data.entries)
        || !data.entries.every(entry => record(entry) && typeof entry.id === 'string' && typeof entry.text === 'string' && ['user', 'assistant', 'tool', 'result', 'status'].includes(String(entry.kind)) && (entry.tool === null || typeof entry.tool === 'string'))) throw new Error('Invalid activity detail')
      return data as unknown as ActivityDetail
    },
  }
}
