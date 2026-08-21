import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'

import type { AgentStatusSocketStatus } from '../../transport/agent-status-socket'

export type AgentActivityMessageKey =
  | 'agents.activity.signInRequired'
  | 'agents.activity.unavailable'
  | 'agents.activity.error'
  | 'agents.activity.running'
  | 'agents.activity.waitingApproval'
  | 'agents.activity.starting'
  | 'agents.activity.stopping'
  | 'agents.activity.idle'
  | 'agents.activity.offline'

export type AgentStreamStatusMessageKey =
  | 'agents.stream.live'
  | 'agents.stream.connecting'
  | 'agents.stream.reconnecting'
  | 'agents.stream.offline'
  | 'agents.stream.stopped'
  | 'agents.stream.notConnected'

export type AgentCountKey = 'windows' | 'runs' | 'subagents'
export type AgentCountMessageKey = `agents.counts.${AgentCountKey}`

export interface AgentCountEntry {
  readonly key: AgentCountKey
  readonly count: number
}

export function agentCountMessageKey(key: AgentCountKey): AgentCountMessageKey {
  return `agents.counts.${key}`
}

export function agentActivityMessageKey(agent: AgentRuntimeSummaryDto): AgentActivityMessageKey {
  if (agent.availability === 'auth_required') return 'agents.activity.signInRequired'
  if (agent.availability === 'unavailable') return 'agents.activity.unavailable'
  if (agent.availability === 'error') return 'agents.activity.error'
  switch (agent.activity) {
    case 'running':
      return 'agents.activity.running'
    case 'waiting_approval':
      return 'agents.activity.waitingApproval'
    case 'starting':
      return 'agents.activity.starting'
    case 'stopping':
      return 'agents.activity.stopping'
    case 'idle':
      return 'agents.activity.idle'
    case 'offline':
      return 'agents.activity.offline'
    case 'error':
      return 'agents.activity.error'
  }
}

export function agentStreamStatusMessageKey(
  status: AgentStatusSocketStatus,
): AgentStreamStatusMessageKey {
  switch (status) {
    case 'connected':
      return 'agents.stream.live'
    case 'connecting':
      return 'agents.stream.connecting'
    case 'reconnecting':
      return 'agents.stream.reconnecting'
    case 'fatal':
      return 'agents.stream.offline'
    case 'closed':
      return 'agents.stream.stopped'
    case 'idle':
      return 'agents.stream.notConnected'
  }
}

export function activeAgentCounts(agent: AgentRuntimeSummaryDto): AgentCountEntry[] {
  const entries: AgentCountEntry[] = [{ key: 'windows', count: agent.active_windows }]
  if (agent.active_runs > 0) entries.push({ key: 'runs', count: agent.active_runs })
  if (agent.active_subagents > 0) entries.push({ key: 'subagents', count: agent.active_subagents })
  return entries
}
