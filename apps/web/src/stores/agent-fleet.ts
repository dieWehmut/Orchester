import type { AgentFleetSnapshotDto } from '@orchester/protokoll'
import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import { normalizeApiError, type ApiError } from '../api/errors'
import type { AgentsApi } from '../api/agents'
import type {
  AgentStatusHeartbeat,
  AgentStatusSocket,
  AgentStatusSocketOptions,
  AgentStatusSocketStatus,
} from '../transport/agent-status-socket'

export type AgentStatusStreamFactory = (
  options: AgentStatusSocketOptions,
) => AgentStatusSocket

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
  const streamStatus = ref<AgentStatusSocketStatus>('idle')
  const lastHeartbeatAt = ref<string | null>(null)
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
  let streamFactory: AgentStatusStreamFactory | null = null
  let stream: AgentStatusSocket | null = null
  let generation = 0
  let streamGeneration = 0

  function configure(
    nextApi: AgentsApi,
    nextStreamFactory?: AgentStatusStreamFactory,
  ): void {
    api = nextApi
    streamFactory = nextStreamFactory ?? null
  }

  function applySnapshot(next: AgentFleetSnapshotDto): void {
    if (snapshot.value && next.sequence < snapshot.value.sequence) return
    snapshot.value = next
    error.value = null
    status.value = 'ready'
  }

  function handleStreamStatus(next: AgentStatusSocketStatus): void {
    streamStatus.value = next
    if (next === 'connected') {
      if (snapshot.value) status.value = 'ready'
      return
    }
    if (next === 'reconnecting' || next === 'fatal') {
      status.value = snapshot.value ? 'stale' : 'error'
    }
  }

  function handleHeartbeat(heartbeat: AgentStatusHeartbeat): void {
    if (snapshot.value && heartbeat.sequence < snapshot.value.sequence) return
    lastHeartbeatAt.value = heartbeat.sent_at
  }

  function closeStream(nextStatus: AgentStatusSocketStatus = 'closed'): void {
    streamGeneration += 1
    const activeStream = stream
    stream = null
    activeStream?.close()
    streamStatus.value = nextStatus
  }

  function connectStream(): void {
    const createStream = streamFactory
    if (!createStream || stream !== null) return
    const currentStreamGeneration = ++streamGeneration
    const nextStream = createStream({
      onSnapshot: (next) => {
        if (currentStreamGeneration !== streamGeneration) return
        applySnapshot(next)
      },
      onHeartbeat: (heartbeat) => {
        if (currentStreamGeneration !== streamGeneration) return
        handleHeartbeat(heartbeat)
      },
      onStatus: (next) => {
        if (currentStreamGeneration !== streamGeneration) return
        handleStreamStatus(next)
      },
      onError: (cause) => {
        if (currentStreamGeneration !== streamGeneration) return
        error.value = normalizeApiError(cause)
        status.value = snapshot.value ? 'stale' : 'error'
      },
    })
    stream = nextStream
    void nextStream.connect().catch(() => undefined)
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
      applySnapshot(next)
    } catch (cause) {
      if (currentGeneration !== generation) return
      error.value = normalizeApiError(cause)
      status.value = snapshot.value ? 'stale' : 'error'
    }
  }

  async function start(): Promise<void> {
    await load()
    connectStream()
  }

  function stop(): void {
    closeStream()
    if (snapshot.value) status.value = 'stale'
  }

  function reset(): void {
    closeStream('idle')
    generation += 1
    status.value = 'idle'
    snapshot.value = null
    error.value = null
    lastHeartbeatAt.value = null
  }

  return {
    status,
    snapshot,
    error,
    streamStatus,
    lastHeartbeatAt,
    runningCount,
    activeSubagentCount,
    activeWindowCount,
    configure,
    applySnapshot,
    load,
    start,
    stop,
    reset,
  }
})
