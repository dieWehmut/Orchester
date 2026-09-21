import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppDrawer from '../src/components/AppDrawer.vue'

afterEach(() => {
  document.body.style.overflow = ''
  document.body.replaceChildren()
})

describe('AppDrawer', () => {
  it('renders on the requested side and locks the document while open', async () => {
    const opener = document.createElement('button')
    document.body.append(opener)
    opener.focus()

    const wrapper = mount(AppDrawer, {
      attachTo: document.body,
      props: { open: true, title: 'Inspector', side: 'right' },
      slots: { default: 'Files and approvals' },
    })
    await nextTick()

    expect(wrapper.find('[role="dialog"]').exists()).toBe(true)
    expect(wrapper.get('[role="dialog"]').classes()).toContain('app-drawer--right')
    expect(document.activeElement).toBe(wrapper.get('[data-drawer-close]').element)
    expect(document.body.style.overflow).toBe('hidden')
  })

  it('closes on Escape and restores the opener focus', async () => {
    const opener = document.createElement('button')
    document.body.append(opener)
    opener.focus()

    const wrapper = mount(AppDrawer, {
      attachTo: document.body,
      props: { open: true, title: 'Inspector', side: 'left' },
    })
    await nextTick()

    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' })
    await nextTick()

    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
    expect(wrapper.emitted('update:open')).toEqual([[false]])
    expect(document.activeElement).toBe(opener)
  })

  it('respects a non-dismissible overlay', async () => {
    const wrapper = mount(AppDrawer, {
      attachTo: document.body,
      props: { open: true, title: 'Inspector', closeOnOverlay: false },
    })
    await nextTick()

    await wrapper.get('.app-drawer__backdrop').trigger('mousedown')
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true)
  })
})
