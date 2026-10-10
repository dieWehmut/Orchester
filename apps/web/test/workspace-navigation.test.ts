import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import WorkspaceNavigation from '../src/components/layout/WorkspaceNavigation.vue'

function mountNavigation() {
  return mount(WorkspaceNavigation, {
    props: {
      label: 'Navigation', homeLabel: 'Home', schedulesLabel: 'Schedules', pluginsLabel: 'Plugins', themeLabel: 'Theme',
      settingsLabel: 'Settings', companionLabel: 'Hide companion',
      activePage: 'workspace', dark: true, companionVisible: true,
    },
  })
}

describe('workspace navigation', () => {
  it('starts a chat and opens settings from the compact navigation', async () => {
    const wrapper = mountNavigation()
    await wrapper.get('[aria-label="Home"]').trigger('click')
    await wrapper.get('[aria-label="Settings"]').trigger('click')
    expect(wrapper.emitted('home')).toHaveLength(1)
    expect(wrapper.emitted('openSettings')).toHaveLength(1)
  })

  it('keeps the requested order and exposes every navigation action', async () => {
    const wrapper = mountNavigation()
    expect(wrapper.findAll('button').map(button => button.attributes('aria-label'))).toEqual(['Home', 'Schedules', 'Plugins', 'Hide companion', 'Theme', 'Settings'])
    await wrapper.get('[aria-label="Schedules"]').trigger('click')
    await wrapper.get('[aria-label="Plugins"]').trigger('click')
    await wrapper.get('[aria-label="Hide companion"]').trigger('click')
    await wrapper.get('[aria-label="Theme"]').trigger('click')
    expect(wrapper.emitted('openSchedules')).toHaveLength(1)
    expect(wrapper.emitted('openPlugins')).toHaveLength(1)
    expect(wrapper.emitted('toggleCompanion')).toHaveLength(1)
    expect(wrapper.emitted('toggleTheme')).toHaveLength(1)
    await wrapper.setProps({ activePage: 'plugins', companionVisible: false, dark: false })
    expect(wrapper.get('[aria-label="Plugins"]').attributes('aria-current')).toBe('page')
    expect(wrapper.get('[aria-label="Hide companion"]').attributes('aria-pressed')).toBe('false')
    expect(wrapper.get('[aria-label="Theme"]').attributes('aria-pressed')).toBe('false')
  })
})
