import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { agentFleetWindowCount, agentPresentedActivity, type AgentCountQuality } from './agent-presenter'

export type AgentPresenceGroupKey = 'active' | 'ready' | 'attention'

export interface AgentFleetGroup {
  readonly key: AgentPresenceGroupKey
  readonly agents: readonly AgentRuntimeSummaryDto[]
  readonly activeWindows: number | null
  readonly activeWindowQuality: AgentCountQuality
}

const GROUP_ORDER: readonly AgentPresenceGroupKey[] = ['active', 'ready', 'attention']

export function agentPresenceGroupKey(agent: AgentRuntimeSummaryDto): AgentPresenceGroupKey {
  const activity = agentPresentedActivity(agent)
  if (agent.availability !== 'available' || activity === 'offline' || activity === 'error' || activity === 'unknown') {
    return 'attention'
  }
  if (activity === 'idle') return 'ready'
  return 'active'
}

export function groupAgentFleet(
  agents: readonly AgentRuntimeSummaryDto[],
): AgentFleetGroup[] {
  return GROUP_ORDER.flatMap((key) => {
    const grouped = agents.filter((agent) => agentPresenceGroupKey(agent) === key)
    const windows = agentFleetWindowCount(grouped)
    return grouped.length === 0
      ? []
      : [
          {
            key,
            agents: grouped,
            activeWindows: windows.count,
            activeWindowQuality: windows.quality,
          },
        ]
  })
}
