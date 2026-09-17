import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppRail from '../src/components/layout/AppRail.vue'

describe('AppRail', () => {
  it('renders the Codex-style rail order: brand, new chat, projects, sessions, fleet', () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: 'Orchester',
        newSessionLabel: 'New chat',
        projectsLabel: 'Projects',
        sessionsLabel: 'Sessions',
        fleetLabel: 'Agents',
        accountName: 'Orchester',
        accountHint: 'Local runtime',
        settingsLabel: 'Settings',
      },
      slots: {
        projects: '<p>Projects</p>',
        sessions: '<p>Sessions</p>',
        fleet: '<p>Agents</p>',
      },
    })

    const sections = wrapper.findAll('[data-rail-section]').map((node) => node.attributes('data-rail-section'))
    expect(sections).toEqual(['brand', 'primary', 'projects', 'sessions', 'fleet', 'account'])
    expect(wrapper.get('[data-rail-section="brand"]').text()).toContain('Orchester')
    expect(wrapper.get('[data-rail-action="new-session"]').text()).toContain('New chat')
  })

  it('emits the new-session intent from the rail action', async () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: null,
        newSessionLabel: 'New chat',
        projectsLabel: 'Projects',
        sessionsLabel: 'Sessions',
        fleetLabel: 'Agents',
        accountName: null,
        settingsLabel: 'Settings',
      },
    })

    await wrapper.get('[data-rail-action="new-session"]').trigger('click')

    expect(wrapper.emitted('newSession')).toHaveLength(1)
  })

  it('pins the account footer under the fleet and emits the settings intent', async () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: 'Orchester',
        newSessionLabel: 'New chat',
        projectsLabel: 'Projects',
        sessionsLabel: 'Sessions',
        fleetLabel: 'Agents',
        accountName: 'Orchester',
        accountHint: 'Local runtime',
        settingsLabel: 'Settings',
      },
      slots: {
        projects: '<p>Projects</p>',
        sessions: '<p>Sessions</p>',
        fleet: '<p>Agents</p>',
      },
    })

    const account = wrapper.get('[data-rail-account]')
    expect(account.text()).toContain('Orchester')
    expect(account.text()).toContain('Local runtime')
    expect(wrapper.get('[data-rail-action=settings]').attributes('aria-label')).toBe('Settings')

    await wrapper.get('[data-rail-action=settings]').trigger('click')

    expect(wrapper.emitted('openSettings')).toHaveLength(1)
  })
})
