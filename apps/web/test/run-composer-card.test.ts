import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('RunComposer Codex-style card', () => {
  it('stacks the prompt above the context and action row', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: 'Ship it',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    const textarea = wrapper.get('textarea').element
    const footer = wrapper.get('[data-composer-footer]').element

    expect(
      textarea.compareDocumentPosition(footer) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('puts the project, approval, model, and send controls in the bottom row', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: 'Ship it',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    const footer = wrapper.get('[data-composer-footer]')
    expect(footer.get('[data-project-context]').text()).toContain('Orchester')
    expect(footer.get('[data-approval-context]').text()).toContain('Ask for approval')
    expect(footer.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(footer.get('[data-composer-action="submit"]').attributes('aria-label')).toBe('Run')
  })
})
