import { describe, expect, it } from 'vitest'

import { HEALTH_SCHEMA_VERSION, type HealthDto } from '../src/api'

describe('health contract', () => {
  it('accepts the redaction-safe service health response', () => {
    const response: HealthDto = {
      status: 'ok',
      service: 'orchester',
      version: '0.1.2',
      schema_version: HEALTH_SCHEMA_VERSION,
    }

    expect(response).toEqual({
      status: 'ok',
      service: 'orchester',
      version: '0.1.2',
      schema_version: HEALTH_SCHEMA_VERSION,
    })
  })
})
