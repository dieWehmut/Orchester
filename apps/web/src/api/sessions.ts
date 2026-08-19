import type { SessionDetailDto, SessionPageDto } from '@orchester/protokoll'

import type { HttpClient } from './http'

export interface SessionListOptions {
  limit?: number
  cursor?: string | null
  signal?: AbortSignal
}

export interface SessionsApi {
  list: (options?: SessionListOptions) => Promise<SessionPageDto>
  detail: (id: string, options?: { signal?: AbortSignal }) => Promise<SessionDetailDto>
}

export function createSessionsApi(http: HttpClient): SessionsApi {
  const withSignal = (signal: AbortSignal | undefined): { signal: AbortSignal } | undefined =>
    signal ? { signal } : undefined

  return {
    list: ({ limit, cursor, signal } = {}) => {
      const query = new URLSearchParams()
      if (limit !== undefined) query.set('limit', String(limit))
      if (cursor) query.set('cursor', cursor)
      const suffix = query.size > 0 ? `?${query.toString()}` : ''
      const requestOptions = withSignal(signal)
      return requestOptions
        ? http.get<SessionPageDto>(`/sessions${suffix}`, requestOptions)
        : http.get<SessionPageDto>(`/sessions${suffix}`)
    },
    detail: (id, { signal } = {}) => {
      const path = `/sessions/${encodeURIComponent(id)}`
      const requestOptions = withSignal(signal)
      return requestOptions
        ? http.get<SessionDetailDto>(path, requestOptions)
        : http.get<SessionDetailDto>(path)
    },
  }
}
