import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { AgentMetrics } from '../src/features/agent-presence'

describe('AgentMetrics', () => {
  it('announces counts with singular and plural labels plus the window source', () => {
    const wrapper = mount(AgentMetrics, {
      props: { agent: AGENT_FLEET_FIXTURE.agents[0]!, variant: 'detail' },
    })

    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('2 windows')
    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('1 subagent')
    expect(wrapper.get('[data-agent-window-source]').text()).toContain('Managed sessions')
  })

  it('keeps the compact variant focused on active counts', () => {
    const wrapper = mount(AgentMetrics, {
      props: { agent: AGENT_FLEET_FIXTURE.agents[1]!, variant: 'compact' },
    })

    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('1 window')
    expect(wrapper.find('[data-agent-window-source]').exists()).toBe(false)
  })
})
