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

  it('collapses the inspector pane from the thread bar panel control', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-pane="inspector"]').attributes('data-inspector-open')).toBe('true')

    await wrapper.get('[data-thread-action="panel"]').trigger('click')

    expect(wrapper.get('[data-pane="inspector"]').attributes('data-inspector-open')).toBe('false')
    expect(wrapper.get('[data-thread-action="panel"]').attributes('aria-pressed')).toBe('false')
  })
})
