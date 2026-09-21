import { describe, expect, it } from 'vitest'

import {
  FRAGMENT_AUTH_SCHEMA_VERSION,
  type FragmentTokenExchangeRequestDto,
  type FragmentTokenExchangeResponseDto,
} from '../src/api'

describe('fragment authentication contract', () => {
  it('sends the fragment token in a one-time POST body', () => {
    const request: FragmentTokenExchangeRequestDto = {
      schema_version: FRAGMENT_AUTH_SCHEMA_VERSION,
      fragment_token: 'one-time-fragment-token',
    }
    const response: FragmentTokenExchangeResponseDto = {
      schema_version: FRAGMENT_AUTH_SCHEMA_VERSION,
      csrf_token: 'csrf-token',
      expires_at: 1_800_000_000,
    }

    expect(request).not.toHaveProperty('query')
    expect(response).not.toHaveProperty('session_cookie')
  })
})
