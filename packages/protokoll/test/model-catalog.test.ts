import { describe, expect, it } from 'vitest'

import {
  MODEL_CATALOG_SCHEMA_VERSION,
  type ModelCatalogDto,
} from '../src/index'

describe('model catalog DTO', () => {
  it('models active, selectable, and unavailable choices without endpoint secrets', () => {
    const catalog: ModelCatalogDto = {
      schema_version: MODEL_CATALOG_SCHEMA_VERSION,
      active: {
        state: 'configured',
        choice: {
          profile: null,
          provider: 'OpenAI',
          provider_name: 'OpenAI',
          model: 'gpt-5.6',
          reasoning_effort: 'high',
          plan_reasoning_effort: null,
          service_tier: 'priority',
        },
      },
      selected_provider: null,
      providers: [
        {
          id: 'OpenAI',
          name: 'OpenAI',
          active: true,
          state: 'selectable',
          model: 'gpt-5.6',
          wire_api: 'responses',
          field: null,
          reason: null,
        },
        {
          id: 'Broken',
          name: 'Broken',
          active: false,
          state: 'unavailable',
          model: null,
          wire_api: null,
          field: 'model_providers.Broken.base_url',
          reason: 'provider base URL is not configured',
        },
      ],
      profiles: [],
    }

    expect(catalog.active.state).toBe('configured')
    expect(catalog.providers[1]?.state).toBe('unavailable')
    const wire = JSON.stringify(catalog)
    expect(wire).not.toContain('base_url":"https://')
    expect(wire).not.toContain('credential')
  })

  it('models an unresolved active choice as bounded validation metadata', () => {
    const catalog: ModelCatalogDto = {
      schema_version: MODEL_CATALOG_SCHEMA_VERSION,
      active: {
        state: 'unresolved',
        field: 'model_provider',
        reason: 'configured provider is unavailable',
      },
      selected_provider: null,
      providers: [],
      profiles: [],
    }

    expect(catalog.active).toEqual({
      state: 'unresolved',
      field: 'model_provider',
      reason: 'configured provider is unavailable',
    })
  })
})
