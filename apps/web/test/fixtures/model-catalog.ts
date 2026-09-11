import {
  MODEL_CATALOG_SCHEMA_VERSION,
  type ModelCatalogDto,
} from '@orchester/protokoll'

export const MODEL_CATALOG_FIXTURE: ModelCatalogDto = {
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
  selected_provider: null,
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
