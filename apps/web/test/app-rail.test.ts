import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppRail from '../src/components/layout/AppRail.vue'
import { shortcutRegistry } from '../src/shortcuts'

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

  it('folds each headed list from its own heading', async () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: 'Orchester',
        newSessionLabel: 'New chat',
        projectsLabel: 'Pinned',
        sessionsLabel: 'Projects',
        fleetLabel: 'Agents',
        accountName: 'Orchester',
        settingsLabel: 'Settings',
      },
      slots: {
        projects: '<p>Pinned list</p>',
        sessions: '<p>Project list</p>',
        fleet: '<p>Agent list</p>',
      },
    })

    // Folding one list must not fold its neighbours, which is the whole reason
    // the heading is the control rather than a header beside a chevron.
    const pinned = wrapper.get('[data-rail-disclosure="pinned"]')
    expect(pinned.attributes('aria-expanded')).toBe('true')
    expect(wrapper.get('[data-rail-section="projects"]').text()).toContain('Pinned list')

    await pinned.trigger('click')

    expect(pinned.attributes('aria-expanded')).toBe('false')
    expect(wrapper.get('[data-rail-section="projects"]').text()).not.toContain('Pinned list')
    expect(wrapper.get('[data-rail-section="sessions"]').text()).toContain('Project list')
    expect(wrapper.get('[data-rail-section="fleet"]').text()).toContain('Agent list')
  })

  it('opens the account menu from the account row and emits its two intents', async () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: 'Orchester',
        newSessionLabel: 'New chat',
        projectsLabel: 'Projects',
        sessionsLabel: 'Sessions',
        fleetLabel: 'Agents',
        accountName: 'dieWehmut',
        accountHint: 'Local runtime',
        settingsLabel: 'Settings',
        companionLabel: 'Hide companion',
      },
    })

    // The reference hangs the menu off the account row rather than leaving a
    // lone gear beside it, so the row itself is the trigger.
    const trigger = wrapper.get('[data-rail-account]').element.closest('button')
    expect(trigger).not.toBeNull()
    await wrapper.get('[data-rail-account-menu] [aria-haspopup="menu"]').trigger('click')

    const items = wrapper.findAll('[role="menuitem"]')
    expect(items.map((item) => item.text())).toEqual(['Hide companion', 'Settings'])

    await items[0]!.trigger('click')
    await wrapper.get('[data-rail-account-menu] [aria-haspopup="menu"]').trigger('click')
    await wrapper.findAll('[role="menuitem"]')[1]!.trigger('click')

    expect(wrapper.emitted('toggleCompanion')).toHaveLength(1)
    expect(wrapper.emitted('openSettings')).toHaveLength(1)
  })

  it('teaches the settings chord on the account menu row, from the live registry', async () => {
    // The settings editor can rebind this chord, so the menu reads it off the
    // registry the shell dispatches from rather than printing the default.
    shortcutRegistry.register({
      id: 'settings.open',
      labelKey: 'shortcuts.labels.settingsOpen',
      groupKey: 'shortcuts.groups.layout',
      keys: ['Mod', ','],
    })

    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: null,
        newSessionLabel: 'New chat',
        projectsLabel: 'Projects',
        sessionsLabel: 'Sessions',
        fleetLabel: 'Agents',
        accountName: 'Orchester',
        settingsLabel: 'Settings',
      },
    })

    await wrapper.get('[data-rail-account-menu] [aria-haspopup="menu"]').trigger('click')

    const settingsRow = wrapper.findAll('[role="menuitem"]')[0]!
    expect(settingsRow.text()).toContain('Ctrl+,')
  })

  it('folds and restores every list from the product row', async () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        workspaceName: 'Orchester',
        newSessionLabel: 'New chat',
        projectsLabel: 'Pinned',
        sessionsLabel: 'Projects',
        fleetLabel: 'Agents',
        accountName: 'Orchester',
        settingsLabel: 'Settings',
      },
      slots: {
        projects: '<p>Pinned list</p>',
        sessions: '<p>Project list</p>',
        fleet: '<p>Agent list</p>',
      },
    })

    // The reference's product row is the parent of the lists under it, so its
    // disclosure clears the column in one gesture; the account row stays put,
    // because that is the row a folded rail is still for.
    const product = wrapper.get('[data-rail-product]')
    expect(product.attributes('aria-expanded')).toBe('true')

    await product.trigger('click')

    expect(product.attributes('aria-expanded')).toBe('false')
    expect(wrapper.find('[data-rail-section="projects"]').exists()).toBe(false)
    expect(wrapper.find('[data-rail-section="sessions"]').exists()).toBe(false)
    expect(wrapper.find('[data-rail-section="fleet"]').exists()).toBe(false)
    expect(wrapper.find('[data-rail-section="account"]').exists()).toBe(true)
    expect(wrapper.get('[data-rail-account]').text()).toContain('Orchester')

    await product.trigger('click')

    expect(wrapper.find('[data-rail-section="projects"]').exists()).toBe(true)
    expect(wrapper.get('[data-rail-section="projects"]').text()).toContain('Pinned list')
  })
})
