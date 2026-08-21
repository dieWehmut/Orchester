import { describe, expect, it } from 'vitest'

import {
  MODEL_CATALOG_SCHEMA_VERSION,
  isModelCatalog,
  parseModelCatalog,
  type ModelCatalogDto,
} from '../src/index'

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
    {
      id: 'relay',
      name: 'Relay',
      active: false,
      state: 'unavailable',
      model: null,
      wire_api: null,
      field: 'model_providers.relay.base_url',
      reason: 'Provider endpoint is not configured',
    },
  ],
  profiles: [
    {
      profile: 'review',
      provider: 'openai',
      provider_name: 'OpenAI',
      model: 'gpt-5.6',
      reasoning_effort: 'medium',
      plan_reasoning_effort: 'high',
      service_tier: null,
    },
  ],
}

describe('model catalog guards', () => {
  it('parses configured models, selectable providers, and profiles', () => {
    expect(parseModelCatalog(catalog)).toEqual(catalog)
    expect(isModelCatalog(catalog)).toBe(true)
  })

  it('rejects unknown fields and contradictory provider states', () => {
    expect(parseModelCatalog({ ...catalog, endpoint: 'https://provider.invalid' })).toBeNull()
    expect(
      parseModelCatalog({
        ...catalog,
        providers: [
          {
            ...catalog.providers[0],
            state: 'unavailable',
            model: 'gpt-5.6',
            wire_api: 'responses',
            field: null,
            reason: null,
          },
        ],
      }),
    ).toBeNull()
  })

  it('rejects duplicate choices and selected providers outside the catalog', () => {
    expect(
      parseModelCatalog({ ...catalog, providers: [catalog.providers[0], catalog.providers[0]] }),
    ).toBeNull()
    expect(parseModelCatalog({ ...catalog, selected_provider: 'missing' })).toBeNull()
    expect(parseModelCatalog({ ...catalog, profiles: [catalog.profiles[0], catalog.profiles[0]] })).toBeNull()
  })

  it('accepts a configured catalog whose selected provider is implicit in the active choice', () => {
    const implicitSelection = { ...catalog, selected_provider: null }

    expect(parseModelCatalog(implicitSelection)).toEqual(implicitSelection)
  })

  it('rejects profiles without a non-empty profile identifier', () => {
    expect(
      parseModelCatalog({
        ...catalog,
        profiles: [{ ...catalog.profiles[0], profile: null }],
      }),
    ).toBeNull()
  })

  it('rejects endpoint, credential, and filesystem details in browser-visible reasons', () => {
    for (const reason of [
      'Provider failed at https://relay.example/v1',
      'api_key=sk-secret',
      'Configuration failed at C:\\Users\\dev\\.orchester\\config.json',
    ]) {
      expect(
        parseModelCatalog({
          ...catalog,
          providers: [{ ...catalog.providers[1], reason }],
          selected_provider: null,
        }),
      ).toBeNull()
    }
  })
})
