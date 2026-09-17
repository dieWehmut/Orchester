import { AGENT_FLEET_FIXTURE, type BootstrapDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

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

    expect(sections).toEqual(['brand', 'primary', 'projects', 'sessions', 'fleet'])
    expect(wrapper.get('[data-rail-section="projects"]').text()).toContain('Orchester')
    expect(wrapper.get('[data-rail-section="sessions"] [data-session-rail]')).toBeTruthy()
    expect(wrapper.get('[data-rail-section="fleet"] [data-agent-fleet]')).toBeTruthy()
  })

  it('starts a new chat from the rail action', async () => {
    const stores = readyStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    await wrapper.get('[data-rail-action="new-session"]').trigger('click')

    expect(stores.sessions.selectedId.value).toBeNull()
  })
})
