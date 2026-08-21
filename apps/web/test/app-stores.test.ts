import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import type { HttpClient } from '../src/api/http'
import { createAppStores } from '../src/stores/app'
import type {
  AgentStatusSocket,
  AgentStatusSocketOptions,
} from '../src/transport/agent-status-socket'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('app stores composition', () => {
  it('shares one HTTP client and in-memory CSRF token across domain stores', () => {
    const stores = createAppStores()

    expect(stores.http).toBe(stores.http)
    expect(stores.runs).toBeDefined()
    expect(stores.agents).toBeDefined()
    expect(stores.models).toBeDefined()
    expect(stores.bootstrap.status.value).toBe('idle')
    expect(stores.sessions.status.value).toBe('idle')
    expect(stores.models.status).toBe('idle')
    expect(stores.getCsrfToken()).toBeNull()
  })

  it('owns the live agent stream across application start and stop', async () => {
    const stream = createFakeStream()
    const stores = createAppStores({
      http: fakeHttp(),
      agentStatusStreamFactory: stream.factory,
    })

    await stores.start()

    expect(stream.connect).toHaveBeenCalledOnce()
    expect(stores.agents.snapshot?.sequence).toBe(12)
    expect(stores.models.activeChoice?.model).toBe('gpt-5.6')
    expect(stores.models.status).toBe('ready')

    stores.stop()

    expect(stream.close).toHaveBeenCalledOnce()
    expect(stores.agents.streamStatus).toBe('closed')
  })
})

function fakeHttp(): HttpClient {
  return {
    request: async () => undefined,
    get: async (path: string) => {
      if (path === '/bootstrap') {
        return {
          schema_version: 1,
          service_version: '0.1.2',
          server_state: 'running',
          workspace: { selected: true, name: 'Orchester' },
        }
      }
      if (path === '/session') {
        return { schema_version: 1, csrf_token: 'csrf', expires_at: 1_800_000_000 }
      }
      if (path === '/agents/status') return AGENT_FLEET_FIXTURE
      if (path === '/models') return MODEL_CATALOG_FIXTURE
      return { schema_version: 1, items: [], next_cursor: null }
    },
    post: async () => undefined,
    put: async () => undefined,
    patch: async () => undefined,
    delete: async () => undefined,
  } as HttpClient
}

function createFakeStream(): {
  connect: ReturnType<typeof vi.fn>
  close: ReturnType<typeof vi.fn>
  factory: (options: AgentStatusSocketOptions) => AgentStatusSocket
} {
  const connect = vi.fn(async () => undefined)
  const close = vi.fn()
  return {
    connect,
    close,
    factory: () => ({ status: 'idle', connect, close }),
  }
}
