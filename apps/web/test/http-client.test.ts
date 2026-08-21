import { describe, expect, it, vi } from 'vitest'

import { createHttpClient } from '../src/api/http'

describe('WebUI HTTP client', () => {
  it('sends same-origin cookies, request IDs, JSON, and the current CSRF token', async () => {
    const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
      new Response(JSON.stringify({ status: 'ok' }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    )
    const client = createHttpClient({
      fetch,
      requestId: () => 'request-123',
      csrfToken: () => 'csrf-456',
    })

    await client.post('/runs', { prompt: 'Inspect the workspace' })

    expect(fetch).toHaveBeenCalledOnce()
    const [url, init] = fetch.mock.calls[0] ?? []
    expect(url).toBe('/api/v1/runs')
    expect(init).toMatchObject({ method: 'POST', credentials: 'same-origin' })
    const headers = new Headers(init?.headers)
    expect(headers.get('content-type')).toBe('application/json')
    expect(headers.get('x-csrf-token')).toBe('csrf-456')
    expect(headers.get('x-request-id')).toBe('request-123')
    expect(init?.body).toBe(JSON.stringify({ prompt: 'Inspect the workspace' }))
  })

  it('forwards AbortSignal and omits CSRF for safe requests', async () => {
    const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
      new Response(JSON.stringify({ status: 'ok' }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    )
    const controller = new AbortController()
    const client = createHttpClient({ fetch, requestId: () => 'request-789' })

    await client.get('/health', { signal: controller.signal })

    const [, init] = fetch.mock.calls[0] ?? []
    const headers = new Headers(init?.headers)
    expect(init?.signal).toBe(controller.signal)
    expect(headers.has('x-csrf-token')).toBe(false)
  })
})
