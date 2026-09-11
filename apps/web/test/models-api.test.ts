import {
  MODEL_CATALOG_SCHEMA_VERSION,
  type ModelCatalogDto,
} from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import type { HttpClient } from '../src/api/http'
import { createModelsApi } from '../src/api/models'

const catalog: ModelCatalogDto = {
  schema_version: MODEL_CATALOG_SCHEMA_VERSION,
  active: {
    state: 'configured',
    choice: {
      profile: null,
      provider: 'openai',
      provider_name: 'OpenAI',
      model: 'gpt-5.6',
      reasoning_effort: 'high',
      plan_reasoning_effort: null,
      service_tier: 'priority',
    },
  },
  selected_provider: 'openai',
  providers: [
    {
      id: 'openai',
      name: 'OpenAI',
      active: true,
      state: 'selectable',
      model: 'gpt-5.6',
      wire_api: 'responses',
      field: null,
      reason: null,
    },
  ],
  profiles: [],
}

describe('model catalog API client', () => {
  it('loads and validates the scoped model catalog', async () => {
    const get = vi.fn(async () => catalog)
    const api = createModelsApi({ get } as unknown as HttpClient)

    await expect(api.catalog()).resolves.toEqual(catalog)
    expect(get).toHaveBeenCalledWith('/models')
  })

  it('forwards an AbortSignal without changing the endpoint', async () => {
    const get = vi.fn(async () => catalog)
    const api = createModelsApi({ get } as unknown as HttpClient)
    const controller = new AbortController()

    await api.catalog({ signal: controller.signal })

    expect(get).toHaveBeenCalledWith('/models', { signal: controller.signal })
  })

  it('rejects malformed and schema-incompatible payloads', async () => {
    const malformed = { ...catalog, schema_version: 99 }
    const api = createModelsApi({ get: vi.fn(async () => malformed) } as unknown as HttpClient)

    await expect(api.catalog()).rejects.toMatchObject({
      code: 'runtime_error',
      retryable: false,
    })
  })
})
