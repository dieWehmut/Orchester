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

  it('lets a selected action keep the focus it takes', async () => {
    const field = document.createElement('input')
    document.body.append(field)

    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Session actions', items, open: true },
      slots: { trigger: '<span>More</span>' },
      attrs: { onSelect: () => field.focus() },
    })
    await nextTick()

    await wrapper.findAll('[role="menuitem"]')[0]?.trigger('click')
    await nextTick()

    expect(document.activeElement).toBe(field)
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

  it('draws an optional header above the rows, outside the menu itself', async () => {
    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Account', items, open: true },
      slots: { header: '<strong>dieWehmut</strong>' },
    })
    await nextTick()

    const header = wrapper.get('[data-menu-header]')
    const menu = wrapper.get('[role="menu"]')

    // The identity belongs to the menu's surface rather than to the menu: a
    // role="menu" may hold items and separators, and nothing else, so a heading
    // inside it would be a list that says it is something it is not.
    expect(
      header.element.compareDocumentPosition(menu.element) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
    expect(menu.findAll('[role="menuitem"]')).toHaveLength(3)
    expect(menu.get('[role="menuitem"]').attributes('role')).toBe('menuitem')
  })

  it('draws no header when the surface has none to give', async () => {
    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Session actions', items, open: true },
    })
    await nextTick()

    expect(wrapper.find('[data-menu-header]').exists()).toBe(false)
  })

  it('announces a menu that chooses as one, and says which item is in force', async () => {
    const choosing = [
      { id: 'low', label: 'Low', checked: false },
      { id: 'high', label: 'High', checked: true },
    ]
    const wrapper = mount(AppMenu, {
      attachTo: document.body,
      props: { label: 'Effort', items: choosing, open: true },
    })
    await nextTick()

    const rows = wrapper.findAll('[role="menuitemradio"]')

    // A menu of actions and a menu of choices announce themselves differently,
    // and only the second says which one is the one in force.
    expect(wrapper.findAll('[role="menuitem"]')).toHaveLength(0)
    expect(rows).toHaveLength(2)
    expect(rows[1]!.attributes('aria-checked')).toBe('true')
    expect(rows[1]!.get('[data-menu-check]')).toBeTruthy()
    expect(rows[0]!.find('[data-menu-check]').exists()).toBe(false)
  })
})
