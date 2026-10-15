import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('RunComposer Codex-style card', () => {
  it('places the prompt above one action row in the rounded field', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: 'Ship it',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    const textarea = wrapper.get('textarea').element
    const action = wrapper.get('[data-composer-footer]').element

    expect(wrapper.find('[data-composer-context]').exists()).toBe(false)
    expect(
      textarea.compareDocumentPosition(action) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('keeps model and permissions inside the input footer', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: 'Ship it',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    const action = wrapper.get('[data-composer-footer]')
    expect(action.find('[data-composer-context]').exists()).toBe(false)
    expect(action.find('[data-model-picker]').exists()).toBe(true)
    expect(action.get('[data-model-context-model]').text()).toContain('gpt-5.6')
    expect(action.find('[data-approval-preset]').exists()).toBe(true)
    expect(action.get('[data-composer-action="submit"]').attributes('aria-label')).toBe('Run')
  })

  it('carries the approval preset in the footer with the run scope', async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: 'Ship it', approvalPreset: 'ask' } })

    const footer = wrapper.get('[data-composer-footer]')
    const preset = footer.get('[data-approval-preset]')
    expect(preset.attributes('data-approval-preset-state')).toBe('ask')

    await preset.get('[aria-haspopup="menu"]').trigger('click')
    const governed = wrapper.findAll('[role="menuitemradio"]').find((item) => item.get('strong').text() === 'Governed')
    await governed?.trigger('click')

    expect(wrapper.emitted('update:approvalPreset')).toEqual([['governed']])
  })
})
