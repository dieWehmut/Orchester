import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ModelContextControl from '../src/components/run/ModelContextControl.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('ModelContextControl', () => {
  it('renders the active model, provider, and effort from the runtime catalog', () => {
    const wrapper = mount(ModelContextControl, {
      props: { catalog: MODEL_CATALOG_FIXTURE, status: 'ready' },
    })

    expect(wrapper.find('[data-model-context]').exists()).toBe(true)
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(wrapper.get('[data-model-context-provider]').text()).toContain('OpenAI')
    expect(wrapper.get('[data-model-context-effort]').text()).toContain('high')
    expect(wrapper.find('[data-model-context-status]').exists()).toBe(false)
  })

  it('keeps the last active context visible while the catalog is stale', () => {
    const wrapper = mount(ModelContextControl, {
      props: { catalog: MODEL_CATALOG_FIXTURE, status: 'stale' },
    })

    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(wrapper.get('[data-model-context-status]').text()).toContain('stale')
  })

  it('shows an unavailable state when no usable model is configured', () => {
    const wrapper = mount(ModelContextControl, {
      props: {
        status: 'error',
        catalog: {
          ...MODEL_CATALOG_FIXTURE,
          active: { state: 'not_configured' },
          selected_provider: null,
          providers: MODEL_CATALOG_FIXTURE.providers.map((provider) => ({
            ...provider,
            active: false,
          })),
        },
      },
    })

    expect(wrapper.get('[data-model-context-unavailable]').text()).toContain('Model unavailable')
    expect(wrapper.get('[data-model-context-unavailable]').attributes('aria-disabled')).toBe('true')
  })
})
