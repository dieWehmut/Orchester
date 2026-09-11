import { AGENT_FLEET_FIXTURE, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { groupAgentFleet } from '../src/features/agent-presence'

describe('agent fleet groups', () => {
  it('orders active agents before attention states and aggregates windows', () => {
    const groups = groupAgentFleet(AGENT_FLEET_FIXTURE.agents)

    expect(groups.map((group) => group.key)).toEqual(['active', 'attention'])
    expect(groups[0]?.agents.map((agent) => agent.agent_id)).toEqual([
      'codex-main',
      'claude-default',
      'deepseek-research',
    ])
    expect(groups[0]?.activeWindows).toBe(4)
    expect(groups[1]?.agents).toHaveLength(2)
  })

  it('keeps an available idle runtime in the ready group', () => {
    const ready: AgentRuntimeSummaryDto = {
      ...AGENT_FLEET_FIXTURE.agents[0]!,
      agent_id: 'codex-ready',
      activity: 'idle',
      active_windows: 0,
      active_runs: 0,
      active_subagents: 0,
    }

    expect(groupAgentFleet([ready])).toEqual([
      { key: 'ready', agents: [ready], activeWindows: 0 },
    ])
  })
})
