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

  it('keeps the project, approval, and model commands out of the action row', () => {
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
    expect(commands.get('[data-approval-context]').text()).toContain('Ask for approval')
    expect(commands.get('[data-model-context-model]').text()).toContain('gpt-5.6')

    const action = wrapper.get('[data-composer-footer]')
    expect(action.find('[data-composer-context]').exists()).toBe(false)
    expect(action.get('[data-composer-action="submit"]').attributes('aria-label')).toBe('Run')
  })
})