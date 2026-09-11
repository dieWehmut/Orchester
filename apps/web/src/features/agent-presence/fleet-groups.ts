import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'

export type AgentPresenceGroupKey = 'active' | 'ready' | 'attention'

export interface AgentFleetGroup {
  readonly key: AgentPresenceGroupKey
  readonly agents: readonly AgentRuntimeSummaryDto[]
  readonly activeWindows: number
}

const GROUP_ORDER: readonly AgentPresenceGroupKey[] = ['active', 'ready', 'attention']

export function agentPresenceGroupKey(agent: AgentRuntimeSummaryDto): AgentPresenceGroupKey {
  if (agent.availability !== 'available' || agent.activity === 'offline' || agent.activity === 'error') {
    return 'attention'
  }
  if (agent.activity === 'idle') return 'ready'
  return 'active'
}

export function groupAgentFleet(
  agents: readonly AgentRuntimeSummaryDto[],
): AgentFleetGroup[] {
  return GROUP_ORDER.flatMap((key) => {
    const grouped = agents.filter((agent) => agentPresenceGroupKey(agent) === key)
    return grouped.length === 0
      ? []
      : [
          {
            key,
            agents: grouped,
            activeWindows: grouped.reduce((sum, agent) => sum + agent.active_windows, 0),
          },
        ]
  })
}
