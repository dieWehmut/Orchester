import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppShell from '../src/components/layout/AppShell.vue'

describe('AppShell narrow-viewport drawers', () => {
  it('keeps the rail reachable through a labelled mobile control', async () => {
    const wrapper = mount(AppShell, {
      props: { sessionsTitle: 'Sessions', controlsLabel: 'Panels' },
      slots: {
        sessions: '<button type="button">Session row</button>',
        default: '<p>Transcript</p>',
      },
    })

    expect(wrapper.find('[data-mobile-controls]').exists()).toBe(true)
    expect(wrapper.get('[data-mobile-sessions]').attributes('aria-label')).toBe('Sessions')

    await wrapper.get('[data-mobile-sessions]').trigger('click')
    expect(wrapper.get('[role="dialog"][aria-labelledby]').text()).toContain('Session row')

    // One drawer, because there is one secondary column now: the shell has no
    // right column, and the run's surfaces are the bottom panel's, which is
    // reachable from the transcript rather than from a drawer.
    expect(wrapper.findAll('[role="dialog"]')).toHaveLength(1)
    expect(wrapper.find('[data-mobile-inspector]').exists()).toBe(false)
  })
})