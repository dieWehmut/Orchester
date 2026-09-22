import { AGENT_FLEET_FIXTURE, type BootstrapDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import { createAppRouter } from '../src/router'
import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'

function readyStores() {
  const stores = createAppStores()
  stores.bootstrap.context.value = {
    schema_version: 1,
    service_version: '0.1.2',
    server_state: 'running',
    workspace: { selected: true, name: 'Orchester' },
  } satisfies BootstrapDto
  stores.bootstrap.status.value = 'ready'
  stores.agents.snapshot = AGENT_FLEET_FIXTURE
  stores.agents.status = 'ready'
  return stores
}

describe('WorkspaceView Codex-style rail', () => {
  it('renders the rail sections with projects and sessions above the fleet', () => {
    const wrapper = mount(WorkspaceView, { global: { plugins: [readyStores()] } })

    const sections = wrapper
      .findAll('[data-pane="sessions"] [data-rail-section]')
      .map((node) => node.attributes('data-rail-section'))

    expect(sections).toEqual(['brand', 'primary', 'projects', 'sessions', 'fleet', 'account'])
    expect(wrapper.get('[data-rail-section="projects"]').text()).toContain('Orchester')
    expect(wrapper.get('[data-rail-section="sessions"] [data-session-rail]')).toBeTruthy()
    expect(wrapper.get('[data-rail-section="fleet"] [data-agent-fleet]')).toBeTruthy()
  })

  it('shows the account footer identity and opens the settings route from its menu', async () => {
    await import('../src/views/SettingsView.vue')
    const router = createAppRouter('memory')
    await router.push('/workspace')
    await router.isReady()
    const wrapper = mount(WorkspaceView, {
      global: { plugins: [readyStores(), router] },
    })

    expect(wrapper.get('[data-rail-account]').text()).toContain('Orchester')
    expect(wrapper.get('[data-rail-account]').text()).toContain('Local runtime')

    // The reference's account row ends at the identity, and its settings row
    // lives in the menu the row opens.
    await wrapper.get('[data-rail-account-menu] [aria-haspopup="menu"]').trigger('click')
    const settingsRow = wrapper
      .findAll('[role="menuitem"]')
      .find((row) => row.text().includes('Settings'))
    expect(settingsRow).toBeDefined()

    await settingsRow!.trigger('click')
    await vi.waitFor(
      () => {
        expect(router.currentRoute.value.name).toBe('settings')
      },
      { timeout: 10_000 },
    )
  })

  it('starts a new chat from the rail action', async () => {
    const stores = readyStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    await wrapper.get('[data-rail-action="new-session"]').trigger('click')

    expect(stores.sessions.selectedId.value).toBeNull()
  })
})
