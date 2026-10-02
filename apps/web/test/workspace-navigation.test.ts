import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import WorkspaceNavigation from '../src/components/layout/WorkspaceNavigation.vue'

function mountNavigation() {
  return mount(WorkspaceNavigation, {
    props: {
      label: 'Workspace panels', newSessionLabel: 'New chat', sessionsLabel: 'Sessions',
      settingsLabel: 'Settings', companionLabel: 'Hide companion',
      bottomPanelLabel: 'Bottom panel', bottomPanelExpanded: false,
      sessionsExpanded: true, homeActive: true, companionVisible: true,
    },
  })
}

describe('workspace navigation', () => {
  it('starts a chat and opens settings from the compact navigation', async () => {
    const wrapper = mountNavigation()
    await wrapper.get('[aria-label="New chat"]').trigger('click')
    await wrapper.get('[aria-label="Settings"]').trigger('click')
    expect(wrapper.emitted('newSession')).toHaveLength(1)
    expect(wrapper.emitted('openSettings')).toHaveLength(1)
  })

  it('reports sidebar and companion states while their buttons remain available', async () => {
    const wrapper = mountNavigation()
    await wrapper.get('[aria-label="Sessions"]').trigger('click')
    await wrapper.get('[aria-label="Hide companion"]').trigger('click')
    await wrapper.get('[aria-label="Bottom panel"]').trigger('click')
    expect(wrapper.emitted('toggleSessions')).toHaveLength(1)
    expect(wrapper.emitted('toggleCompanion')).toHaveLength(1)
    expect(wrapper.emitted('toggleBottomPanel')).toHaveLength(1)
    await wrapper.setProps({ sessionsExpanded: false, companionVisible: false, homeActive: false })
    expect(wrapper.get('[aria-label="Sessions"]').attributes('aria-expanded')).toBe('false')
    expect(wrapper.get('[aria-label="Hide companion"]').attributes('aria-pressed')).toBe('false')
    expect(wrapper.get('[aria-label="New chat"]').attributes('aria-pressed')).toBe('false')
  })
})
