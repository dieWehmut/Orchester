import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'

describe('WorkspaceView thread bar', () => {
  it('renders the Codex-style thread bar above the transcript', () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    const bar = wrapper.get('[data-pane="transcript"] [data-thread-bar]')
    expect(bar.get('[data-thread-title]').text()).toBe('New chat')
    expect(bar.get('[data-thread-action="panel"]')).toBeTruthy()
  })

  it('names the thread and the agent it belongs to, with the agent\'s state', () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    stores.bootstrap.context.value = {
      schema_version: 1,
      service_version: '0.1.2',
      server_state: 'running',
      workspace: { selected: true, name: 'Orchester' },
    }
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    const bar = wrapper.get('[data-pane="transcript"] [data-thread-bar]')

    // The reference puts an identity above the title: who is answering, and
    // whether they are reachable right now.
    expect(bar.find('[data-thread-agent]').exists()).toBe(true)
    expect(bar.find('[data-thread-status]').exists()).toBe(true)
  })

  it('carries a labelled share action, not a bare icon', () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    const share = wrapper.get('[data-pane="transcript"] [data-thread-action="share"]')
    expect(share.text().length).toBeGreaterThan(0)
  })

  it('opens and closes the bottom panel from the thread bar panel control', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('collapsed')

    await wrapper.get('[data-thread-action="panel"]').trigger('click')

    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('expanded')
    expect(wrapper.get('[data-thread-action="panel"]').attributes('aria-pressed')).toBe('true')

    await wrapper.get('[data-thread-action="panel"]').trigger('click')

    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('collapsed')
    expect(wrapper.get('[data-thread-action="panel"]').attributes('aria-pressed')).toBe('false')
  })
})
