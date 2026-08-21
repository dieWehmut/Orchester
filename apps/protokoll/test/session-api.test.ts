import { describe, expect, it } from 'vitest'

import {
  SESSION_SCHEMA_VERSION,
  type SessionBootstrapDto,
} from '../src/api'

describe('browser session contract', () => {
  it('returns a csrf token and expiry without returning a session secret', () => {
    const response: SessionBootstrapDto = {
      schema_version: SESSION_SCHEMA_VERSION,
      csrf_token: 'csrf-token-for-browser',
      expires_at: 1_800_000_000,
    }

    expect(response.csrf_token).toBeTruthy()
    expect(response.expires_at).toBeGreaterThan(0)
    expect(response).not.toHaveProperty('session_token')
  })
})
