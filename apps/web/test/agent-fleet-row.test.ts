import {
  AGENT_FLEET_FIXTURE,
  type AgentRuntimeSummaryDto,
} from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AgentFleetRow from '../src/components/agents/AgentFleetRow.vue'

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
})
