import {
  AGENT_FLEET_FIXTURE,
  type AgentFleetSnapshotDto,
} from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createAgentsApi } from '../src/api/agents'
import type { HttpClient } from '../src/api/http'

describe('agent status API client', () => {
  it('loads a versioned fleet snapshot from the scoped status endpoint', async () => {
    const get = vi.fn(async () => AGENT_FLEET_FIXTURE)
    const api = createAgentsApi({ get } as unknown as HttpClient)

    await expect(api.status()).resolves.toEqual(AGENT_FLEET_FIXTURE)
    expect(get).toHaveBeenCalledWith('/agents/status')
  })

  it('forwards AbortSignal without changing the endpoint contract', async () => {
    const get = vi.fn(async () => AGENT_FLEET_FIXTURE)
    const api = createAgentsApi({ get } as unknown as HttpClient)
    const controller = new AbortController()

    await api.status({ signal: controller.signal })

    expect(get).toHaveBeenCalledWith('/agents/status', { signal: controller.signal })
  })

  it('rejects malformed or schema-incompatible server payloads', async () => {
    const malformed = { ...AGENT_FLEET_FIXTURE, schema_version: 99 } as unknown as AgentFleetSnapshotDto
    const api = createAgentsApi({ get: vi.fn(async () => malformed) } as unknown as HttpClient)

    await expect(api.status()).rejects.toMatchObject({
      code: 'runtime_error',
      retryable: false,
    })
  })
})
