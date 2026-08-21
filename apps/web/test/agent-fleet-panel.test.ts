import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AgentFleetPanel from '../src/components/agents/AgentFleetPanel.vue'

describe('AgentFleetPanel', () => {
  it('shows every provider icon, activity label, managed windows, and subagent count', () => {
    const wrapper = mount(AgentFleetPanel, {
      props: { status: 'ready', snapshot: AGENT_FLEET_FIXTURE },
    })

    expect(wrapper.get('[data-agent-fleet]')).toBeTruthy()
    expect(wrapper.findAll('[data-agent-id]')).toHaveLength(5)
    expect(wrapper.get('[data-agent-id="codex-main"] [data-agent-icon="codex"]')).toBeTruthy()
    expect(wrapper.get('[data-agent-id="claude-default"] [data-agent-icon="claude"]')).toBeTruthy()
    expect(wrapper.get('[data-agent-id="deepseek-research"] [data-agent-icon="deepseek"]')).toBeTruthy()
    expect(wrapper.get('[data-agent-id="codex-main"] [data-active-windows]').text()).toBe('2')
    expect(wrapper.get('[data-agent-id="deepseek-research"] [data-active-subagents]').text()).toBe('2')
    expect(wrapper.get('[data-agent-id="deepseek-research"] [data-agent-activity]').text()).toContain('Running')
  })

  it('keeps unavailable and auth-required agents visible with an explanatory state', () => {
    const wrapper = mount(AgentFleetPanel, {
      props: { status: 'ready', snapshot: AGENT_FLEET_FIXTURE },
    })

    expect(wrapper.get('[data-agent-id="opencode-local"] [data-agent-activity]').text()).toContain('Unavailable')
    expect(wrapper.get('[data-agent-id="claude-team"] [data-agent-activity]').text()).toContain('Sign in required')
  })

  it('renders a recoverable error without discarding the last snapshot', () => {
    const wrapper = mount(AgentFleetPanel, {
      props: { status: 'stale', snapshot: AGENT_FLEET_FIXTURE, error: 'Runtime is offline' },
    })

    expect(wrapper.get('[data-agent-fleet-stale]').text()).toContain('Runtime is offline')
    expect(wrapper.findAll('[data-agent-id]')).toHaveLength(5)
  })

  it('shows whether agent status updates are live or reconnecting', async () => {
    const wrapper = mount(AgentFleetPanel, {
      props: {
        status: 'ready',
        streamStatus: 'connected',
        snapshot: AGENT_FLEET_FIXTURE,
      },
    })

    expect(wrapper.get('[data-agent-stream-status]').text()).toBe('Live')

    await wrapper.setProps({ streamStatus: 'reconnecting' })

    expect(wrapper.get('[data-agent-stream-status]').text()).toBe('Reconnecting')
  })

  it('uses the selected locale for fleet status copy', async () => {
    const wrapper = mount(AgentFleetPanel, {
      props: { status: 'ready', snapshot: AGENT_FLEET_FIXTURE },
    })

    expect(wrapper.get('[data-agent-fleet-title]').text()).toBe('Agents')
    expect(wrapper.get('[data-agent-id="codex-main"] [data-agent-activity]').text()).toContain('Running')
  })
})
