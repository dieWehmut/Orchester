import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ComposerContextBar from '../src/components/run/ComposerContextBar.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('ComposerContextBar', () => {
  it('shows project, approval policy, and active model context', () => {
    const wrapper = mount(ComposerContextBar, {
      props: {
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    expect(wrapper.get('[data-project-context]').text()).toContain('Orchester')
    expect(wrapper.get('[data-approval-context]').text()).toContain('Ask for approval')
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(wrapper.findAll('svg').length).toBeGreaterThanOrEqual(3)
  })

  it('renders explicit fallbacks while project and model context are unavailable', () => {
    const wrapper = mount(ComposerContextBar, {
      props: { workspaceName: null, modelCatalog: null, modelStatus: 'loading' },
    })

    expect(wrapper.get('[data-project-context]').text()).toContain('Choose project')
    expect(wrapper.get('[data-model-context-unavailable]').text()).toContain('Model unavailable')
    expect(wrapper.get('[data-model-context-status]').text()).toContain('loading')
  })

  it('does not expose non-functional project or model buttons', () => {
    const wrapper = mount(ComposerContextBar, {
      props: {
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    expect(wrapper.findAll('button')).toHaveLength(0)
  })
})
