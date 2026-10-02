import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'
import { hasShellAction, runShellAction } from '../src/components/layout/shell-actions'

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

  it('offers a working chat menu and omits sharing until the runtime supports it', async () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.find('[data-thread-action="share"]').exists()).toBe(false)
    stores.sessions.selectedId.value = 'previous-session'
    await wrapper.get('[data-thread-action="more"] [aria-haspopup="menu"]').trigger('click')
    const newChat = wrapper.findAll('[role="menuitem"]').find(row => row.text() === 'New chat')
    expect(newChat).toBeDefined()
    await newChat!.trigger('click')
    expect(stores.sessions.selectedId.value).toBeNull()
  })

  it('opens the inspector from the chrome action and closes it from the thread bar', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-pane="inspector"]').attributes('data-inspector-open')).toBe('false')

    expect(hasShellAction('inspector.toggle')).toBe(true)
    expect(runShellAction('inspector.toggle')).toBe(true)
    await wrapper.vm.$nextTick()
    expect(wrapper.get('[data-pane="inspector"]').attributes('data-inspector-open')).toBe('true')
    expect(wrapper.get('[role="dialog"]').text()).toContain('No agent selected')

    await wrapper.get('[data-thread-action="panel"]').trigger('click')

    expect(wrapper.get('[data-pane="inspector"]').attributes('data-inspector-open')).toBe('false')
    expect(wrapper.get('[data-thread-action="panel"]').attributes('aria-pressed')).toBe('false')
    wrapper.unmount()
    expect(hasShellAction('inspector.toggle')).toBe(false)
  })
})
