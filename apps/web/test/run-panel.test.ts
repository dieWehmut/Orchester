import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'
import RunPanel from '../src/components/run/RunPanel.vue'

describe('RunPanel', () => {
  it('renders an actionable empty run with composer and footer', () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })

    expect(wrapper.get('[data-run-panel]')).toBeTruthy()
    expect(wrapper.get('[data-run-composer]')).toBeTruthy()
    expect(wrapper.get('[data-run-footer]')).toBeTruthy()
    expect(wrapper.get('[data-empty-workspace]')).toBeTruthy()
    expect(wrapper.get('[data-orchester-mark]')).toBeTruthy()
  })

  it('removes the large mark immediately after a conversation starts', async () => {
    const wrapper = mount(RunPanel, {
      props: { view: createEmptyRunView(), conversationStarted: false },
    })

    expect(wrapper.find('[data-orchester-mark]').exists()).toBe(true)
    await wrapper.setProps({ conversationStarted: true, busy: true })

    expect(wrapper.find('[data-orchester-mark]').exists()).toBe(false)
    expect(wrapper.get('[data-run-awaiting-events]')).toBeTruthy()
    expect(wrapper.get('[data-run-composer]')).toBeTruthy()
  })

  it('forwards submit and cancel intents without fetching', async () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })
    const textarea = wrapper.get('textarea')

    await textarea.setValue('Inspect the workspace')
    await textarea.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('submit')).toEqual([['Inspect the workspace']])

    await wrapper.setProps({ busy: true })
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('cancel')).toHaveLength(1)
  })
})
