import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('RunComposer Codex-style card', () => {
  it('stacks the command row above the prompt and the action row below it', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: 'Ship it',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    const commands = wrapper.get('[data-composer-context]').element
    const textarea = wrapper.get('textarea').element
    const action = wrapper.get('[data-composer-footer]').element

    expect(
      commands.compareDocumentPosition(textarea) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
    expect(
      textarea.compareDocumentPosition(action) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('separates the fact about the workspace from the decisions about the run', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: 'Ship it',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    const commands = wrapper.get('[data-composer-context]')
    expect(commands.get('[data-project-context]').text()).toContain('Orchester')
    // The model is not a fact about the workspace: it is a choice about the next
    // run, so it is drawn at the field's trailing edge with the action, which is
    // where the reference draws it. This reverses the earlier separation, and
    // the reference is what this surface is being built against.
    expect(commands.find('[data-model-context]').exists()).toBe(false)
    expect(commands.find('[data-approval-preset]').exists()).toBe(false)

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
    const governed = wrapper.findAll('[role="menuitem"]').find((item) => item.text() === 'Governed')
    await governed?.trigger('click')

    expect(wrapper.emitted('update:approvalPreset')).toEqual([['governed']])
  })
})
