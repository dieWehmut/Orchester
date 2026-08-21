import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { AgentDetails } from '../src/features/agent-presence'

describe('AgentDetails', () => {
  it('renders a neutral empty state when no agent is selected', () => {
    const wrapper = mount(AgentDetails, { props: { agent: null } })

    expect(wrapper.get('[data-agent-details-empty]').text()).toContain('No agent selected')
  })

  it('renders identity, status, counts, and capabilities for Codex', () => {
    const agent = AGENT_FLEET_FIXTURE.agents[0]
    if (!agent) throw new Error('fixture must contain Codex')
    const wrapper = mount(AgentDetails, { props: { agent } })

    expect(wrapper.get('[data-agent-details]')).toBeTruthy()
    expect(wrapper.get('[data-agent-details-icon="codex"]')).toBeTruthy()
    expect(wrapper.get('[data-agent-details-name]').text()).toBe('Codex')
    expect(wrapper.get('[data-agent-details-activity]').text()).toContain('Running')
    expect(wrapper.get('[data-agent-detail="windows"] [data-agent-detail-value]').text()).toBe('2')
    expect(wrapper.get('[data-agent-detail="runs"] [data-agent-detail-value]').text()).toBe('2')
    expect(wrapper.get('[data-agent-detail="subagents"] [data-agent-detail-value]').text()).toBe('1')
    expect(wrapper.get('[data-agent-capabilities]').text()).toContain('streaming')
    expect(wrapper.get('[data-agent-capabilities]').text()).toContain('subagents')
  })
})
