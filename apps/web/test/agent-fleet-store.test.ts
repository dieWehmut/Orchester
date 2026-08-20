import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { AgentsApi } from '../src/api/agents'
import { useAgentFleetStore } from '../src/stores/agent-fleet'

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
})
