import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'

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
})
