import { describe, expect, it } from 'vitest'

import type { ApiErrorCode, ApiErrorDto } from '../src/api'

describe('API error contract', () => {
  it('uses stable codes and keeps the message independent from secrets', () => {
    const code: ApiErrorCode = 'unauthorized'
    const response: ApiErrorDto = {
      error: 'authentication is required',
      code,
      request_id: 'request-123',
      retryable: false,
    }

    expect(response.code).toBe('unauthorized')
    expect(response.error).not.toContain('token')
  })
})
