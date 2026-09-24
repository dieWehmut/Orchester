import { AGENT_FLEET_FIXTURE, type BootstrapDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import { resetPinnedSessionsForTests } from '../src/composables/use-pinned-sessions'
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
  it('renders the rail sections with the pinned list and the sessions above the fleet', () => {
    const wrapper = mount(WorkspaceView, { global: { plugins: [readyStores()] } })

    const sections = wrapper
      .findAll('[data-pane="sessions"] [data-rail-section]')
      .map((node) => node.attributes('data-rail-section'))

    expect(sections).toEqual(['brand', 'primary', 'projects', 'sessions', 'fleet', 'account'])
    // The first list is the reader's own, as the reference's sidebar opens. With
    // nothing pinned it is a heading and nothing else: the control that adds one
    // says so on every row it sits beside, and the reference leaves that space
    // empty too.
    expect(wrapper.find('[data-rail-section="projects"] [data-pinned-empty]').exists()).toBe(false)
    expect(wrapper.find('[data-rail-section="projects"] [data-pinned-sessions]').exists()).toBe(
      false,
    )
    // The product row names the product; the workspace's own state moved to the
    // field's project control and the projects list, which are where a reader
    // changes it.
    expect(wrapper.get('[data-rail-section="brand"]').text()).toContain('Orchester')
    expect(wrapper.get('[data-rail-section="sessions"] [data-session-rail]')).toBeTruthy()
    expect(wrapper.get('[data-rail-section="fleet"] [data-agent-fleet]')).toBeTruthy()
  })

  it('pins a run from its row into the first list, and unpins it again', async () => {
    localStorage.clear()
    resetPinnedSessionsForTests()
    const stores = readyStores()
    stores.sessions.items.value = [
      {
        id: 's-11111111111111111111111111111111',
        source: 'delegate',
        recorded_at_unix: 1_700_000_000,
        title: 'Inspect the runtime',
        agent: 'codex',
        model: 'gpt-5',
        outcome: 'success',
        resumable: true,
      },
    ]
    stores.sessions.status.value = 'ready'

    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    await nextTick()

    await wrapper.get('[data-session-pin]').trigger('click')
    await nextTick()

    // The reader's own list, above the recent ones: the reference opens with it,
    // and it is the reader's ordering rather than a fact about the run.
    const pinned = wrapper.get('[data-rail-section="projects"]')
    expect(pinned.get('[data-pinned-sessions]').text()).toContain('Inspect the runtime')
    // A run appears once: the recent list is not a second copy of what was kept.
    expect(wrapper.find('[data-rail-section="sessions"] [data-session-id]').exists()).toBe(false)

    await pinned.get('[data-session-pin]').trigger('click')
    await nextTick()

    // Unpinning empties the reader's list, and the run goes back to the projects
    // list it came from.
    expect(
      wrapper.find('[data-rail-section="projects"] [data-pinned-sessions]').exists(),
    ).toBe(false)
    expect(wrapper.find('[data-rail-section="sessions"] [data-session-id]').exists()).toBe(true)
    resetPinnedSessionsForTests()
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
