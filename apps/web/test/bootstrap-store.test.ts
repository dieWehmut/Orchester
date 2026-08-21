import type { BootstrapDto, SessionBootstrapDto } from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import type { HttpClient } from '../src/api/http'
import { createBootstrapStore } from '../src/stores/bootstrap'

const bootstrap: BootstrapDto = {
  schema_version: 1,
  service_version: '0.1.2',
  server_state: 'running',
  workspace: { selected: true, name: 'Orchester' },
}

const session: SessionBootstrapDto = {
  schema_version: 1,
  csrf_token: 'csrf-in-memory',
  expires_at: 1_800_000_000,
}

function httpStub(): HttpClient {
  return {
    request: vi.fn(),
    get: vi.fn(async (path: string) => (path === '/bootstrap' ? bootstrap : session)),
    post: vi.fn(async () => session),
    put: vi.fn(),
    patch: vi.fn(),
    delete: vi.fn(),
  } as HttpClient
}

describe('WebUI bootstrap store', () => {
  it('removes a one-time fragment before exchanging it and keeps CSRF out of state', async () => {
    const http = httpStub()
    const replaceState = vi.fn()
    const installCsrfToken = vi.fn()
    const store = createBootstrapStore({
      http,
      location: {
        hash: '#fragment_token=one-time-secret',
        pathname: '/workspace',
        search: '?source=desktop',
      },
      history: { state: { boot: true }, replaceState },
      installCsrfToken,
    })

    await store.load()

    expect(replaceState).toHaveBeenCalledWith({ boot: true }, '', '/workspace?source=desktop')
    expect(http.post).toHaveBeenCalledWith('/auth/fragment', {
      schema_version: 1,
      fragment_token: 'one-time-secret',
    })
    expect(installCsrfToken).toHaveBeenCalledWith('csrf-in-memory')
    expect(store.status.value).toBe('ready')
    expect(store.context.value).toEqual(bootstrap)
    expect(JSON.stringify(store)).not.toContain('csrf-in-memory')
    expect(JSON.stringify(store)).not.toContain('one-time-secret')
  })

  it('opens a normal cookie session and exposes normalized failures', async () => {
    const http = httpStub()
    const store = createBootstrapStore({
      http,
      location: { hash: '', pathname: '/workspace', search: '' },
      history: { state: null, replaceState: vi.fn() },
      installCsrfToken: vi.fn(),
    })

    await store.load()
    expect(http.get).toHaveBeenCalledWith('/session')

    vi.mocked(http.get).mockRejectedValueOnce(new TypeError('Failed to fetch'))
    await store.load()
    expect(store.status.value).toBe('error')
    expect(store.error.value).toMatchObject({ code: 'network', retryable: true })
  })
})
