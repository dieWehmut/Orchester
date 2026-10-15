import { mount, flushPromises } from '@vue/test-utils'
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
    await wrapper.get('[data-composer-action="cancel"]').trigger('click')

    expect(wrapper.emitted('submit')).toBeUndefined()
    expect(wrapper.emitted('cancel')).toHaveLength(1)
    expect(wrapper.find('button[type="submit"]').exists()).toBe(false)
  })

  it('keeps attachment and permissions left, model and mic before the send action', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: '',
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    expect(wrapper.find('[data-composer-context]').exists()).toBe(false)
    const footer = wrapper.get('[data-composer-footer]')
    const items = ['[data-composer-attach]', '[data-approval-preset]', '[data-model-context]', '[data-composer-mic]', '[data-composer-action="submit"]']
    for (let i = 1; i < items.length; i++) {
      expect(footer.get(items[i-1]!).element.compareDocumentPosition(footer.get(items[i]!).element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    }
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
  })

  it('attaches text separately, includes it on submit and preserves the draft after an invalid attachment', async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: 'Read this' } })
    const input = wrapper.get('input[type="file"]')
    const file = new File(['hello'], 'notes.md', { type: 'text/plain' })
    Object.defineProperty(file, 'text', { value: async () => 'hello' })
    Object.defineProperty(input.element, 'files', { configurable: true, value: [file] })
    await input.trigger('change'); await flushPromises()
    expect(wrapper.get('textarea').element.value).toBe('Read this')
    expect(wrapper.get('[data-composer-attachments]').text()).toContain('notes.md')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('submit')?.[0]?.[0]).toBe('Read this\n\nnotes.md\n```\nhello\n```')
    Object.defineProperty(input.element, 'files', { configurable: true, value: [new File(['binary'], 'photo.png')] })
    await input.trigger('change'); await flushPromises()
    expect(wrapper.get('[role="status"]').text()).toContain('text files')
    expect(wrapper.get('textarea').element.value).toBe('Read this')
    expect(wrapper.get('[data-composer-attachments]').text()).toContain('notes.md')
  })
})
