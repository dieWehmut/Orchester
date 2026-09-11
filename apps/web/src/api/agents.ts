import {
  parseAgentFleetSnapshot,
  type AgentFleetSnapshotDto,
} from '@orchester/protokoll'

import { ApiError } from './errors'
import type { HttpClient } from './http'

export interface AgentStatusOptions {
  signal?: AbortSignal
}

export interface AgentsApi {
  status: (options?: AgentStatusOptions) => Promise<AgentFleetSnapshotDto>
}

export function createAgentsApi(http: HttpClient): AgentsApi {
  return {
    async status({ signal } = {}): Promise<AgentFleetSnapshotDto> {
      const raw = signal
        ? await http.get<unknown>('/agents/status', { signal })
        : await http.get<unknown>('/agents/status')
      const snapshot = parseAgentFleetSnapshot(raw)
      if (snapshot === null) {
        throw new ApiError('Invalid agent status response', {
          code: 'runtime_error',
          retryable: false,
        })
      }
      return snapshot
    },
  }
}
