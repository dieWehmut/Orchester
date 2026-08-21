import {
  AGENT_FLEET_FIXTURE,
  type AgentRuntimeSummaryDto,
} from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { AgentFleetRow } from '../src/features/agent-presence'

describe('AgentFleetRow', () => {
  it('renders a fallback icon for an unknown provider key and emits the agent id', async () => {
    const source = AGENT_FLEET_FIXTURE.agents[0]
    if (!source) throw new Error('fixture must contain Codex')
    const agent: AgentRuntimeSummaryDto = { ...source, icon_key: 'custom-provider' }
    const wrapper = mount(AgentFleetRow, { props: { agent } })

    expect(wrapper.get('[data-agent-icon="generic"]')).toBeTruthy()
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('select')).toEqual([[agent.agent_id]])
  })

  it('renders windows, active runs, and subagent counts as separate metrics', () => {
    const source = AGENT_FLEET_FIXTURE.agents[0]
    if (!source) throw new Error('fixture must contain Codex')
    const wrapper = mount(AgentFleetRow, { props: { agent: source } })

    expect(wrapper.get('[data-active-windows]').text()).toBe('2')
    expect(wrapper.get('[data-active-runs]').text()).toBe('2')
    expect(wrapper.get('[data-active-subagents]').text()).toBe('1')
    expect(wrapper.findAll('[data-agent-count]')).toHaveLength(3)
  })

  it('exposes selected state through button semantics', () => {
    const source = AGENT_FLEET_FIXTURE.agents[0]
    if (!source) throw new Error('fixture must contain Codex')
    const wrapper = mount(AgentFleetRow, { props: { agent: source, selected: true } })

    expect(wrapper.get('button').attributes('aria-pressed')).toBe('true')
    expect(wrapper.get('button').classes()).toContain('agent-fleet-row--selected')
  })
})
