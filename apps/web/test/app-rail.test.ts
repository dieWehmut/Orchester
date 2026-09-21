import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppRail from '../src/components/layout/AppRail.vue'

describe('AppRail', () => {
  it('answers the reference sidebar with a titled product row and headed lists', () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: 'Orchester',
        newSessionLabel: 'New chat',
        projectsLabel: 'Pinned',
        sessionsLabel: 'Projects',
        fleetLabel: 'Agents',
        accountName: 'dieWehmut',
        accountHint: 'Local runtime',
        settingsLabel: 'Settings',
      },
    })

    // The reference names the product row and keeps a disclosure beside it, so
    // the sidebar reads as a product switcher rather than a bare logo.
    const product = wrapper.get('[data-rail-product]')
    expect(product.text()).toContain('Orchester')
    expect(product.get('[data-rail-product-disclosure]')).toBeTruthy()

    // Every list is headed, so a reader can fold one without folding the rest,
    // and the headings follow the reference's order.
    const headings = wrapper
      .findAll('[data-rail-heading]')
      .map((node) => node.attributes('data-rail-heading'))
    expect(headings).toEqual(['pinned', 'projects', 'agents'])
  })

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
