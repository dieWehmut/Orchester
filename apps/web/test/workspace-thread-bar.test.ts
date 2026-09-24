import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'

const writes: string[] = []

afterEach(() => {
  writes.length = 0
  vi.restoreAllMocks()
})

function stubClipboard(): void {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: {
      writeText: (value: string) => {
        writes.push(value)
        return Promise.resolve()
      },
    },
  })
}

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

  it('copies the conversation from the thread bar, and says it did', async () => {
    // The reference's share publishes the thread and hands back a link. There is
    // no host here to publish to, so the control is named for what it does - and
    // it was a control that emitted into nothing until this wave. What the text
    // *is* has its own test; this one is the wiring.
    stubClipboard()
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    await nextTick()

    const share = wrapper.get('[data-pane="transcript"] [data-thread-action="share"]')
    expect(share.text()).toContain('Copy conversation')

    await share.trigger('click')
    await nextTick()

    // The conversation is copied (empty here, because nothing has been said) and
    // the control says so while it is saying it.
    expect(writes).toHaveLength(1)
    expect(share.text()).toContain('Copied')
  })
})
