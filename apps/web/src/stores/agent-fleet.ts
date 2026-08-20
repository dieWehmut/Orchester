import type { AgentFleetSnapshotDto } from '@orchester/protokoll'
import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import { normalizeApiError, type ApiError } from '../api/errors'
import type { AgentsApi } from '../api/agents'

export type AgentFleetStoreStatus =
  | 'idle'
  | 'loading'
  | 'refreshing'
  | 'ready'
  | 'stale'
  | 'error'

export const useAgentFleetStore = defineStore('agentFleet', () => {
  const status = ref<AgentFleetStoreStatus>('idle')
  const snapshot = shallowRef<AgentFleetSnapshotDto | null>(null)
  const error = shallowRef<ApiError | null>(null)
  const runningCount = computed(
    () => snapshot.value?.agents.filter((agent) => agent.activity === 'running').length ?? 0,
  )
  const activeSubagentCount = computed(
    () => snapshot.value?.agents.reduce((total, agent) => total + agent.active_subagents, 0) ?? 0,
  )
  const activeWindowCount = computed(
    () => snapshot.value?.agents.reduce((total, agent) => total + agent.active_windows, 0) ?? 0,
  )

  let api: AgentsApi | null = null
  let generation = 0

  function configure(nextApi: AgentsApi): void {
    api = nextApi
  }

  async function load(): Promise<void> {
    const currentApi = api
    const currentGeneration = ++generation
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('agent status API unavailable'))
      status.value = snapshot.value ? 'stale' : 'error'
      return
    }
    status.value = snapshot.value ? 'refreshing' : 'loading'
    error.value = null
    try {
      const next = await currentApi.status()
      if (currentGeneration !== generation) return
      snapshot.value = next
      status.value = 'ready'
    } catch (cause) {
      if (currentGeneration !== generation) return
      error.value = normalizeApiError(cause)
      status.value = snapshot.value ? 'stale' : 'error'
    }
  }

  function reset(): void {
    generation += 1
    status.value = 'idle'
    snapshot.value = null
    error.value = null
  }

  return {
    status,
    snapshot,
    error,
    runningCount,
    activeSubagentCount,
    activeWindowCount,
    configure,
    load,
    reset,
  }
})
