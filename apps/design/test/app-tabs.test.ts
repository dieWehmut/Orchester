import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppTabs from '../src/components/AppTabs.vue'

const tabs = [
  { id: 'run', label: 'Run' },
  { id: 'files', label: 'Files', disabled: true },
  { id: 'settings', label: 'Settings' },
]

afterEach(() => {
  document.body.replaceChildren()
})

describe('AppTabs', () => {
  it('renders a named tablist with one selected and one roving tab stop', () => {
    const wrapper = mount(AppTabs, {
      attachTo: document.body,
      props: { modelValue: 'run', tabs, ariaLabel: 'Inspector views' },
    })
    const rendered = wrapper.findAll('[role="tab"]')

    expect(wrapper.attributes('role')).toBe('tablist')
    expect(wrapper.attributes('aria-label')).toBe('Inspector views')
    expect(rendered).toHaveLength(3)
    expect(rendered[0]?.attributes()).toMatchObject({
      'aria-selected': 'true',
      tabindex: '0',
    })
    expect(rendered[1]?.attributes()).toMatchObject({
      'aria-selected': 'false',
      disabled: '',
      tabindex: '-1',
    })
  })

  it('skips disabled tabs and focuses the next enabled tab', async () => {
    const wrapper = mount(AppTabs, {
      attachTo: document.body,
      props: { modelValue: 'run', tabs, ariaLabel: 'Inspector views' },
    })
    const first = wrapper.findAll('[role="tab"]')[0]

    await first?.trigger('keydown', { key: 'ArrowRight' })
    await nextTick()

    expect(wrapper.emitted('update:modelValue')).toEqual([['settings']])
    expect(document.activeElement).toBe(wrapper.findAll('[role="tab"]')[2]?.element)
  })

  it('supports Home and End in vertical mode', async () => {
    const verticalTabs = [
      { id: 'one', label: 'One' },
      { id: 'two', label: 'Two' },
      { id: 'three', label: 'Three' },
    ]
    const wrapper = mount(AppTabs, {
      attachTo: document.body,
      props: { modelValue: 'two', tabs: verticalTabs, ariaLabel: 'Steps', orientation: 'vertical' },
    })
    const current = wrapper.findAll('[role="tab"]')[1]

    await current?.trigger('keydown', { key: 'End' })
    await nextTick()
    await wrapper.findAll('[role="tab"]')[2]?.trigger('keydown', { key: 'Home' })
    await nextTick()

    expect(wrapper.emitted('update:modelValue')).toEqual([['three'], ['one']])
  })
})
