import type { AgentRuntimeSummaryDto } from './agent-status'

export type AgentPresenceCountUnit = 'windows' | 'processes' | 'services' | 'instances'
export type AgentPresenceCountQuality = 'exact' | 'lower_bound' | 'unknown'

export function agentProcessSourcesComplete(agent: AgentRuntimeSummaryDto): boolean {
  return agent.process_sources !== null && agent.process_sources.length > 0 &&
    agent.process_sources.every((source) => source.status === 'available')
}

export function agentIsMonitorOnly(agent: AgentRuntimeSummaryDto): boolean {
  return agent.capabilities.includes('monitor_only')
}

export function agentHasManagedActivity(agent: AgentRuntimeSummaryDto): boolean {
  return agent.active_sessions > 0 || agent.active_runs > 0 || agent.active_subagents > 0 ||
    (agent.window_count_source !== 'external_processes' && agent.active_windows > 0)
}

export function agentPresentedActivity(agent: AgentRuntimeSummaryDto): AgentRuntimeSummaryDto['activity'] {
  if ((agent.activity === 'idle' || agent.activity === 'offline') && !agentHasManagedActivity(agent) && !agentProcessSourcesComplete(agent)) return 'unknown'
  return agent.activity
}

export function agentWindowCount(agent: AgentRuntimeSummaryDto): { count: number | null; quality: AgentPresenceCountQuality } {
  if (agentProcessSourcesComplete(agent) || (agent.window_count_source !== 'external_processes' && agent.active_windows > 0)) {
    return { count: agent.active_windows, quality: 'exact' }
  }
  return agent.active_windows > 0
    ? { count: agent.active_windows, quality: 'lower_bound' }
    : { count: null, quality: 'unknown' }
}

export function agentFleetWindowCount(agents: readonly AgentRuntimeSummaryDto[]): { count: number | null; quality: AgentPresenceCountQuality } {
  const count = agents.reduce((total, agent) => total + (agentWindowCount(agent).count ?? 0), 0)
  if (agents.length > 0 && agents.every(agentProcessSourcesComplete)) return { count, quality: 'exact' }
  return count > 0 ? { count, quality: 'lower_bound' } : { count: null, quality: 'unknown' }
}

export function agentPresenceCountKey(agent: AgentRuntimeSummaryDto): Exclude<AgentPresenceCountUnit, 'instances'> {
  if (agent.window_count_source !== 'external_processes') return 'windows'
  return agent.capabilities.includes('service_presence') ? 'services' : 'processes'
}

export function agentFleetCountKey(agents: readonly AgentRuntimeSummaryDto[]): AgentPresenceCountUnit {
  const first = agents[0]
  if (!first) return 'instances'
  const key = agentPresenceCountKey(first)
  return agents.every((agent) => agentPresenceCountKey(agent) === key) ? key : 'instances'
}
