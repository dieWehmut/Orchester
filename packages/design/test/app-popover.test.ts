import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppPopover from '../src/components/AppPopover.vue'

afterEach(() => {
  document.body.replaceChildren()
})

describe('AppPopover', () => {
  it('opens from its anchor and exposes the selected placement', async () => {
    const wrapper = mount(AppPopover, {
      attachTo: document.body,
      props: { title: 'Run details', placement: 'bottom' },
      slots: {
        anchor: '<button type="button">Details</button>',
        default: '<p>Usage and status</p>',
      },
    })

    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
    await wrapper.find('button').trigger('click')
    await nextTick()

    expect(wrapper.find('[role="dialog"]').exists()).toBe(true)
    expect(wrapper.get('[role="dialog"]').classes()).toContain('app-popover--bottom')
    expect(wrapper.emitted('update:open')).toEqual([[true]])
  })

  it('closes on Escape and when the pointer lands outside', async () => {
    const wrapper = mount(AppPopover, {
      attachTo: document.body,
      props: { open: true, title: 'Run details' },
      slots: {
        anchor: '<button type="button">Details</button>',
        default: '<p>Usage and status</p>',
      },
    })
    await nextTick()

    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' })
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)

    await wrapper.find('button').trigger('click')
    await nextTick()
    document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))
    await nextTick()

    expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
    expect(wrapper.emitted('update:open')).toContainEqual([false])
  })

  it('does not close when a pointer event starts inside the popover root', async () => {
    const wrapper = mount(AppPopover, {
      attachTo: document.body,
      props: { open: true, title: 'Run details' },
      slots: {
        anchor: '<button type="button">Details</button>',
        default: '<p>Usage and status</p>',
      },
    })
    await nextTick()

    await wrapper.get('[role="dialog"]').trigger('pointerdown')
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true)
  })
})
