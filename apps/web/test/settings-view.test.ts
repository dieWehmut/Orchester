import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import SettingsView from '../src/views/SettingsView.vue'

describe('SettingsView', () => {
  it('renders a sectioned settings surface with a navigation list', () => {
    const wrapper = mount(SettingsView)

    const sections = wrapper
      .findAll('[data-settings-section]')
      .map((node) => node.attributes('data-settings-section'))

    expect(sections).toEqual(['general', 'appearance', 'providers', 'about'])
    expect(wrapper.get('[data-settings-nav]')).toBeTruthy()
    expect(wrapper.get('[data-settings-nav]').text()).toContain('Appearance')
  })

  it('keeps the appearance controls reachable from the settings navigation', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-settings-nav-link="appearance"]').trigger('click')

    expect(wrapper.get('[data-settings-section="appearance"]').attributes('aria-selected')).toBe('true')
  })
})
