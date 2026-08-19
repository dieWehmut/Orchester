import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import WorkspaceResponsive from '../src/components/layout/WorkspaceResponsive.vue'

describe('WorkspaceResponsive', () => {
  it('keeps both secondary surfaces reachable through labelled mobile controls', async () => {
    const wrapper = mount(WorkspaceResponsive, {
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
    expect(wrapper.findAll('[role="dialog"]')).toHaveLength(2)
  })
})
