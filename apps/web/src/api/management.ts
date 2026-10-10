import type { HttpClient } from './http'

export interface ScheduleInput {
  name: string
  prompt: string
  enabled: boolean
  first_run_at: number
  interval_seconds: number | null
}
export interface ScheduleRun {
  id: string
  run_id: string | null
  started_at: number
  finished_at: number | null
  status: 'running' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted'
  error_code: string | null
}
export interface Schedule extends ScheduleInput {
  id: string
  next_run_at: number
  created_at: number
  updated_at: number
  history: ScheduleRun[]
}
export interface Plugin {
  name: string
  display_name: string
  package_name: string
  version: string
  installed: boolean
  origin: 'managed' | 'project' | null
  removable: boolean
}

export function createManagementApi(http: HttpClient) {
  return {
    async schedules(signal?: AbortSignal): Promise<Schedule[]> {
      const data = await http.get<{ schema_version: number; schedules: Schedule[] }>('/schedules', signal ? { signal } : {})
      if (data.schema_version !== 1 || !Array.isArray(data.schedules)) throw new Error('Invalid schedules response')
      return data.schedules
    },
    save: (input: ScheduleInput, id?: string) => id
      ? http.put<Schedule>(`/schedules/${encodeURIComponent(id)}`, input)
      : http.post<Schedule>('/schedules', input),
    removeSchedule: (id: string) => http.delete<void>(`/schedules/${encodeURIComponent(id)}`),
    run: (id: string) => http.post<ScheduleRun>(`/schedules/${encodeURIComponent(id)}/runs`),
    cancel: (runId: string) => http.post(`/runs/${encodeURIComponent(runId)}/cancel`),
    async plugins(signal?: AbortSignal): Promise<Plugin[]> {
      const data = await http.get<{ schema_version: number; plugins: Plugin[] }>('/plugins', signal ? { signal } : {})
      if (data.schema_version !== 1 || !Array.isArray(data.plugins)) throw new Error('Invalid plugins response')
      return data.plugins
    },
    installPlugin: (name: string) => http.post('/plugins', { name }),
    removePlugin: (name: string) => http.delete(`/plugins/${encodeURIComponent(name)}`),
  }
}

export function scheduleInput(task: Schedule): ScheduleInput {
  return { name: task.name, prompt: task.prompt, enabled: task.enabled, first_run_at: task.first_run_at, interval_seconds: task.interval_seconds }
}
