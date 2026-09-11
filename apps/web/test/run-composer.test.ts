import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('RunComposer', () => {
  it('submits on Enter but keeps Shift+Enter as a newline', async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: 'Inspect the tree' } })
    const textarea = wrapper.get('textarea')

    await textarea.trigger('keydown', { key: 'Enter' })
    await textarea.trigger('keydown', { key: 'Enter', shiftKey: true })

    expect(wrapper.emitted('submit')).toEqual([['Inspect the tree']])
  })

  it('does not submit empty or over-limit prompts', async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: ' ', maxLength: 3 } })
    const textarea = wrapper.get('textarea')

    await textarea.trigger('keydown', { key: 'Enter' })
    await wrapper.setProps({ modelValue: 'abcd' })
    await textarea.trigger('keydown', { key: 'Enter' })

    expect(wrapper.emitted('submit')).toBeUndefined()
    expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined()
  })

  it('shows a stop action while busy and suppresses submit', async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: 'Run', busy: true } })
    const textarea = wrapper.get('textarea')

    await textarea.trigger('keydown', { key: 'Enter' })
    await wrapper.get('button').trigger('click')

    expect(wrapper.emitted('submit')).toBeUndefined()
    expect(wrapper.emitted('cancel')).toHaveLength(1)
    expect(wrapper.find('button[type="submit"]').exists()).toBe(false)
  })

  it('renders the runtime project, approval, and model context above the prompt', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: '',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    expect(wrapper.get('[data-composer-context]').element.compareDocumentPosition(
      wrapper.get('textarea').element,
    ) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(wrapper.get('[data-project-context]').text()).toContain('Orchester')
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
  })
})
