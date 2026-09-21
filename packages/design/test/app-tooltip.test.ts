import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'

import AppTooltip from '../src/components/AppTooltip.vue'

afterEach(() => {
  vi.useRealTimers()
  document.body.replaceChildren()
})

describe('AppTooltip', () => {
  it('shows on focus after the delay and exposes a description relationship', async () => {
    vi.useFakeTimers()
    const wrapper = mount(AppTooltip, {
      attachTo: document.body,
      props: { content: 'Open settings', delay: 100 },
      slots: { trigger: '<button type="button">Settings</button>' },
    })
    const trigger = wrapper.find('button')

    await trigger.trigger('focus')
    expect(wrapper.find('[role="tooltip"]').exists()).toBe(false)
    await vi.advanceTimersByTimeAsync(99)
    expect(wrapper.find('[role="tooltip"]').exists()).toBe(false)
    await vi.advanceTimersByTimeAsync(1)
    await nextTick()

    const tooltip = wrapper.get('[role="tooltip"]')
    expect(tooltip.text()).toBe('Open settings')
    expect(wrapper.attributes('aria-describedby')).toBe(tooltip.attributes('id'))
  })

  it('shows on pointer entry, hides on leave, and closes on Escape', async () => {
    const wrapper = mount(AppTooltip, {
      attachTo: document.body,
      props: { content: 'More actions', delay: 0 },
      slots: { trigger: '<button type="button">More</button>' },
    })

    await wrapper.trigger('pointerenter')
    await nextTick()
    expect(wrapper.find('[role="tooltip"]').exists()).toBe(true)

    await wrapper.trigger('keydown', { key: 'Escape' })
    expect(wrapper.find('[role="tooltip"]').exists()).toBe(false)

    await wrapper.trigger('pointerenter')
    await nextTick()
    await wrapper.trigger('pointerleave')
    expect(wrapper.find('[role="tooltip"]').exists()).toBe(false)
  })

  it('marks the trigger as reduced-motion when the OS requests it', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    )
    mount(AppTooltip, {
      attachTo: document.body,
      props: { content: 'Info' },
      slots: { trigger: '<button type="button">Info</button>' },
    })
    await nextTick()

    expect(document.querySelector('.app-tooltip--reduced-motion')).not.toBeNull()
  })
})
