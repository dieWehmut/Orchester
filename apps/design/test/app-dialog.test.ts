import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppDialog from '../src/components/AppDialog.vue'

afterEach(() => {
  document.body.style.overflow = ''
  document.body.replaceChildren()
})

describe('AppDialog', () => {
  it('focuses the close control and traps Tab within the dialog', async () => {
    const opener = document.createElement('button')
    opener.textContent = 'Open'
    document.body.append(opener)
    opener.focus()

    const wrapper = mount(AppDialog, {
      attachTo: document.body,
      props: { open: true, title: 'Run approval' },
      slots: {
        default: 'Review this action',
        footer: '<button data-action type="button">Approve</button>',
      },
    })
    await nextTick()

    const dialog = wrapper.get('[role="dialog"]')
    const close = wrapper.get('[data-dialog-close]')
    const action = wrapper.get('[data-action]')

    expect(dialog.attributes('aria-modal')).toBe('true')
    expect(document.activeElement).toBe(close.element)
    expect(document.body.style.overflow).toBe('hidden')

    ;(action.element as HTMLButtonElement).focus()
    await dialog.trigger('keydown', { key: 'Tab' })
    expect(document.activeElement).toBe(close.element)

    await close.trigger('keydown', { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(action.element)
  })

  it('closes on Escape and restores focus to the opener', async () => {
    const opener = document.createElement('button')
    opener.textContent = 'Open'
    document.body.append(opener)
    opener.focus()

    const wrapper = mount(AppDialog, {
      attachTo: document.body,
      props: { open: true, title: 'Run approval' },
    })
    await nextTick()

    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' })
    await nextTick()

    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
    expect(wrapper.emitted('update:open')).toEqual([[false]])
    expect(document.activeElement).toBe(opener)
    expect(document.body.style.overflow).toBe('')
  })

  it('closes when the backdrop is clicked, but not when the dialog body is clicked', async () => {
    const wrapper = mount(AppDialog, {
      attachTo: document.body,
      props: { open: true, title: 'Run approval' },
    })
    await nextTick()

    await wrapper.get('[role="dialog"]').trigger('click')
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true)

    await wrapper.get('.app-dialog__backdrop').trigger('mousedown')
    await nextTick()
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
  })
})
