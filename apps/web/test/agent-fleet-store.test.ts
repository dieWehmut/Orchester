import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { AgentsApi } from '../src/api/agents'
import { useAgentFleetStore } from '../src/stores/agent-fleet'
import type {
  AgentStatusSocket,
  AgentStatusSocketOptions,
} from '../src/transport/agent-status-socket'

describe('agent fleet Pinia store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('keeps each Pinia instance isolated and derives active counts', async () => {
    const first = useAgentFleetStore()
    const api = { status: vi.fn(async () => AGENT_FLEET_FIXTURE) } as unknown as AgentsApi

    first.configure(api)
    await first.load()

    setActivePinia(createPinia())
    const second = useAgentFleetStore()

    expect(first.snapshot?.sequence).toBe(12)
    expect(first.runningCount).toBe(2)
    expect(first.activeSubagentCount).toBe(3)
    expect(second.snapshot).toBeNull()
  })

  it('retains the last snapshot and marks it stale after a refresh failure', async () => {
    const store = useAgentFleetStore()
    const api = {
      status: vi
        .fn()
        .mockResolvedValueOnce(AGENT_FLEET_FIXTURE)
        .mockRejectedValueOnce(new TypeError('offline')),
    } as unknown as AgentsApi

    store.configure(api)
    await store.load()
    await store.load()

    expect(store.snapshot?.sequence).toBe(12)
    expect(store.status).toBe('stale')
    expect(store.error?.message).toBe('Unable to reach the Orchester runtime')
  })

  it('resets data and errors without affecting the API object', async () => {
    const store = useAgentFleetStore()
    const api = { status: vi.fn(async () => AGENT_FLEET_FIXTURE) } as unknown as AgentsApi
    store.configure(api)
    await store.load()

    store.reset()

    expect(store.snapshot).toBeNull()
    expect(store.status).toBe('idle')
    expect(store.error).toBeNull()
    expect(api.status).toHaveBeenCalledTimes(1)
  })

  it('starts a live status stream after the REST snapshot and accepts newer snapshots', async () => {
    const store = useAgentFleetStore()
    const api = { status: vi.fn(async () => AGENT_FLEET_FIXTURE) } as unknown as AgentsApi
    const stream = createFakeStream()

    store.configure(api, stream.factory)
    await store.start()

    expect(stream.connect).toHaveBeenCalledOnce()
    expect(store.snapshot?.sequence).toBe(12)

    stream.options?.onSnapshot?.({
      ...AGENT_FLEET_FIXTURE,
      sequence: 13,
      agents: AGENT_FLEET_FIXTURE.agents.map((agent) =>
        agent.agent_id === 'codex-main'
          ? { ...agent, activity: 'running', active_windows: 4 }
          : agent,
      ),
    })

    expect(store.snapshot?.sequence).toBe(13)
    expect(store.activeWindowCount).toBe(6)
    expect(store.status).toBe('ready')
    expect(store.error).toBeNull()
  })

  it('ignores older stream snapshots and marks retained data stale while reconnecting', async () => {
    const store = useAgentFleetStore()
    const api = { status: vi.fn(async () => AGENT_FLEET_FIXTURE) } as unknown as AgentsApi
    const stream = createFakeStream()

    store.configure(api, stream.factory)
    await store.start()
    stream.options?.onSnapshot?.({ ...AGENT_FLEET_FIXTURE, sequence: 11 })
    stream.options?.onStatus?.('reconnecting')
    stream.options?.onError?.(new TypeError('socket offline'))

    expect(store.snapshot?.sequence).toBe(12)
    expect(store.streamStatus).toBe('reconnecting')
    expect(store.status).toBe('stale')
    expect(store.error?.message).toBe('Unable to reach the Orchester runtime')
  })

  it('closes its live stream when reset', async () => {
    const store = useAgentFleetStore()
    const api = { status: vi.fn(async () => AGENT_FLEET_FIXTURE) } as unknown as AgentsApi
    const stream = createFakeStream()

    store.configure(api, stream.factory)
    await store.start()
    store.reset()

    expect(stream.close).toHaveBeenCalledOnce()
    expect(store.streamStatus).toBe('idle')
  })

  it('still opens the live stream when the initial REST snapshot is unavailable', async () => {
    const store = useAgentFleetStore()
    const api = { status: vi.fn(async () => Promise.reject(new TypeError('offline'))) } as unknown as AgentsApi
    const stream = createFakeStream()

    store.configure(api, stream.factory)
    await store.start()

    expect(stream.connect).toHaveBeenCalledOnce()
    expect(store.status).toBe('error')

    stream.options?.onSnapshot?.(AGENT_FLEET_FIXTURE)

    expect(store.snapshot?.sequence).toBe(12)
    expect(store.status).toBe('ready')
  })
})

function createFakeStream(): {
  options: AgentStatusSocketOptions | null
  connect: ReturnType<typeof vi.fn>
  close: ReturnType<typeof vi.fn>
  factory: (options: AgentStatusSocketOptions) => AgentStatusSocket
} {
  const result = {
    options: null as AgentStatusSocketOptions | null,
    connect: vi.fn(async () => undefined),
    close: vi.fn(),
    factory: (options: AgentStatusSocketOptions): AgentStatusSocket => {
      result.options = options
      return {
        status: 'idle',
        connect: result.connect,
        close: result.close,
      }
    },
  }
  return result
}
