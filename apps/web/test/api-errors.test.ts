import { describe, expect, it } from 'vitest'

import { normalizeApiError } from '../src/api/errors'
import { HttpResponseError } from '../src/api/http'

describe('WebUI API error normalization', () => {
  it('preserves the stable server error contract and retry hint', () => {
    const error = normalizeApiError(
      new HttpResponseError(
        new Response(null, { status: 503 }),
        {
          error: 'Workspace is unavailable',
          code: 'unavailable',
          request_id: 'request-123',
          retryable: true,
        },
      ),
    )

    expect(error).toMatchObject({
      message: 'Workspace is unavailable',
      code: 'unavailable',
      requestId: 'request-123',
      retryable: true,
      status: 503,
    })
  })

  it('classifies HTTP, network, and abort failures without exposing raw payloads', () => {
    expect(
      normalizeApiError(new HttpResponseError(new Response('conflict', { status: 409 }), 'conflict')),
    ).toMatchObject({ code: 'conflict', retryable: false, status: 409 })
    expect(normalizeApiError(new TypeError('Failed to fetch'))).toMatchObject({
      code: 'network',
      retryable: true,
    })
    expect(normalizeApiError(new DOMException('cancelled', 'AbortError'))).toMatchObject({
      code: 'aborted',
      retryable: false,
    })
  })
})
