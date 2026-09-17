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
      },
      slots: {
        projects: '<p>Projects</p>',
        sessions: '<p>Sessions</p>',
        fleet: '<p>Agents</p>',
      },
    })

    const sections = wrapper.findAll('[data-rail-section]').map((node) => node.attributes('data-rail-section'))
    expect(sections).toEqual(['brand', 'primary', 'projects', 'sessions', 'fleet'])
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
      },
    })

    await wrapper.get('[data-rail-action="new-session"]').trigger('click')

    expect(wrapper.emitted('newSession')).toHaveLength(1)
  })
})
