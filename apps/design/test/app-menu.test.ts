import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppMenu from '../src/components/AppMenu.vue'

const items = [
  { id: 'rename', label: 'Rename' },
  { id: 'delete', label: 'Delete', disabled: true },
  { id: 'archive', label: 'Archive' },
]

afterEach(() => {
  document.body.replaceChildren()
})

describe('AppMenu', () => {
  it('opens from its trigger and focuses the first enabled item', async () => {
    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Session actions', items },
      slots: { trigger: '<span>More</span>' },
    })
    const trigger = wrapper.get('[aria-haspopup="menu"]')

    expect(wrapper.find('[role="menu"]').exists()).toBe(false)

    await trigger.trigger('click')
    await nextTick()

    expect(wrapper.find('[role="menu"]').exists()).toBe(true)
    expect(document.activeElement).toBe(wrapper.findAll('[role="menuitem"]')[0]?.element)
    expect(wrapper.emitted('update:open')).toEqual([[true]])
  })

  it('skips disabled items, selects an item, and restores trigger focus on Escape', async () => {
    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Session actions', items, open: true },
      slots: { trigger: '<span>More</span>' },
    })
    await nextTick()
    const menuItems = wrapper.findAll('[role="menuitem"]')

    await menuItems[0]?.trigger('keydown', { key: 'ArrowDown' })
    await nextTick()
    expect(document.activeElement).toBe(menuItems[2]?.element)

    await menuItems[2]?.trigger('click')
    await nextTick()
    expect(wrapper.emitted('select')).toEqual([['archive']])
    expect(wrapper.emitted('update:open')).toContainEqual([false])
    expect(document.activeElement).toBe(wrapper.get('[aria-haspopup="menu"]').element)
  })

  it('closes when the pointer lands outside the menu root', async () => {
    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Session actions', items, open: true },
    })
    await nextTick()

    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))
    await nextTick()

    expect(wrapper.find('[role="menu"]').exists()).toBe(false)
    expect(wrapper.emitted('update:open')).toContainEqual([false])
  })
})
