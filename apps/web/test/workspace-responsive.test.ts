import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppShell from '../src/components/layout/AppShell.vue'

describe('AppShell narrow-viewport drawers', () => {
  it('keeps both secondary surfaces reachable through labelled mobile controls', async () => {
    const wrapper = mount(AppShell, {
      props: { sessionsTitle: 'Sessions', inspectorTitle: 'Inspector', controlsLabel: 'Panels' },
      slots: {
        sessions: '<button type="button">Session row</button>',
        default: '<p>Transcript</p>',
        inspector: '<button type="button">Approval row</button>',
      },
    })

    expect(wrapper.find('[data-mobile-controls]').exists()).toBe(true)
    expect(wrapper.get('[data-mobile-sessions]').attributes('aria-label')).toBe('Sessions')
    expect(wrapper.get('[data-mobile-inspector]').attributes('aria-label')).toBe('Inspector')

    await wrapper.get('[data-mobile-sessions]').trigger('click')
    expect(wrapper.get('[role="dialog"][aria-labelledby]').text()).toContain('Session row')

    await wrapper.get('[data-mobile-inspector]').trigger('click')
    expect(wrapper.findAll('[role="dialog"]')).toHaveLength(1)
    expect(wrapper.get('[role="dialog"]').text()).toContain('Approval row')
    expect(wrapper.emitted('update:inspectorOpen')?.at(-1)).toEqual([true])
  })

  it('opens a controlled inspector as a drawer and follows it back to the desktop dock', async () => {
    const originalWidth = window.innerWidth
    window.innerWidth = 960
    const wrapper = mount(AppShell, {
      props: { sessionsTitle: 'Sessions', inspectorTitle: 'Inspector', inspectorOpen: false },
      slots: { inspector: '<p>Review changes</p>' },
    })
    try {
      expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
      await wrapper.setProps({ inspectorOpen: true })
      expect(wrapper.get('[role="dialog"]').text()).toContain('Review changes')

      await wrapper.get('[data-drawer-close]').trigger('click')
      expect(wrapper.emitted('update:inspectorOpen')?.at(-1)).toEqual([false])
      await wrapper.setProps({ inspectorOpen: false })
      await wrapper.setProps({ inspectorOpen: true })
      window.innerWidth = 1280
      window.dispatchEvent(new Event('resize'))
      await wrapper.vm.$nextTick()
      expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
      expect(wrapper.get('[data-inspector]').attributes('data-inspector-open')).toBe('true')
    } finally {
      wrapper.unmount()
      window.innerWidth = originalWidth
    }
  })
})
