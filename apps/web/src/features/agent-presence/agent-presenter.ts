import { agentHasManagedActivity, agentPresenceCountKey, agentPresentedActivity, agentWindowCount, type AgentRuntimeSummaryDto, type AgentPresenceCountQuality, type AgentPresenceCountUnit } from '@orchester/protokoll'
export { agentFleetCountKey, agentFleetWindowCount, agentIsMonitorOnly, agentPresenceCountKey, agentPresentedActivity, agentProcessSourcesComplete, agentWindowCount } from '@orchester/protokoll'
import type { I18n } from '../../i18n'

import type { AgentStatusSocketStatus } from '../../transport/agent-status-socket'

export type AgentActivityMessageKey =
  | 'agents.activity.signInRequired'
  | 'agents.activity.unavailable'
  | 'agents.activity.error'
  | 'agents.activity.running'
  | 'agents.activity.serviceRunning'
  | 'agents.activity.noRunningProcesses'
  | 'agents.activity.waitingApproval'
  | 'agents.activity.starting'
  | 'agents.activity.stopping'
  | 'agents.activity.idle'
  | 'agents.activity.offline'
  | 'agents.activity.unknown'

export type AgentStreamStatusMessageKey =
  | 'agents.stream.live'
  | 'agents.stream.connecting'
  | 'agents.stream.reconnecting'
  | 'agents.stream.offline'
  | 'agents.stream.stopped'
  | 'agents.stream.notConnected'

export type AgentAvailabilityMessageKey =
  | 'agents.availability.available'
  | 'agents.availability.unavailable'
  | 'agents.availability.authRequired'
  | 'agents.availability.error'

export type AgentPresenceCountKey = AgentPresenceCountUnit
export type AgentCountKey = AgentPresenceCountKey | 'runs' | 'subagents'
export type AgentCountMessageKey =
  | 'agents.counts.window'
  | 'agents.counts.windows'
  | 'agents.counts.process'
  | 'agents.counts.processes'
  | 'agents.counts.service'
  | 'agents.counts.services'
  | 'agents.counts.instance'
  | 'agents.counts.instances'
  | 'agents.counts.run'
  | 'agents.counts.runs'
  | 'agents.counts.subagent'
  | 'agents.counts.subagents'

export type AgentWindowSourceMessageKey =
  | 'agents.windowSource.managedSessions'
  | 'agents.windowSource.desktopWindows'
  | 'agents.windowSource.externalProcesses'

export interface AgentCountEntry {
  readonly key: AgentCountKey
  readonly count: number | null
  readonly quality?: AgentCountQuality
}

export type AgentCountQuality = AgentPresenceCountQuality

export function agentCountValue(entry: Pick<AgentCountEntry, 'count' | 'quality'>, t: I18n['t']): string {
  if (entry.count === null) return '—'
  return entry.quality === 'lower_bound' ? t('agents.counts.atLeast', { count: String(entry.count) }) : String(entry.count)
}

export function agentCountsSummary(agent: AgentRuntimeSummaryDto, t: I18n['t']): string {
  return activeAgentCounts(agent).map((entry) => {
    const value = entry.count === null ? t('agents.activity.unknown') : agentCountValue(entry, t)
    return `${value} ${t(agentCountMessageKey(entry.key, entry.count ?? 2))}`
  }).join(', ')
}

export function agentCountMessageKey(key: AgentCountKey, count = 2): AgentCountMessageKey {
  if (count === 1) {
    if (key === 'windows') return 'agents.counts.window'
    if (key === 'processes') return 'agents.counts.process'
    if (key === 'services') return 'agents.counts.service'
    if (key === 'instances') return 'agents.counts.instance'
    if (key === 'runs') return 'agents.counts.run'
    return 'agents.counts.subagent'
  }
  return `agents.counts.${key}`
}

export function agentWindowSourceMessageKey(
  source: AgentRuntimeSummaryDto['window_count_source'],
): AgentWindowSourceMessageKey {
  if (source === 'tauri_windows') return 'agents.windowSource.desktopWindows'
  if (source === 'external_processes') return 'agents.windowSource.externalProcesses'
  return 'agents.windowSource.managedSessions'
}

export function agentActivityMessageKey(agent: AgentRuntimeSummaryDto): AgentActivityMessageKey {
  const activity = agentPresentedActivity(agent)
  if ((agent.capabilities.includes('process_presence') || agent.window_count_source === 'external_processes') && !agentHasManagedActivity(agent)) {
    if (activity === 'running') return 'agents.activity.serviceRunning'
    if (activity === 'idle' || activity === 'offline') return 'agents.activity.noRunningProcesses'
  }
  if (activity === 'idle' || activity === 'offline') {
    if (agent.availability === 'auth_required') return 'agents.activity.signInRequired'
    if (agent.availability === 'unavailable') return 'agents.activity.unavailable'
    if (agent.availability === 'error') return 'agents.activity.error'
  }
  switch (activity) {
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
    case 'unknown':
      return 'agents.activity.unknown'
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

export function agentAvailabilityMessageKey(
  agent: AgentRuntimeSummaryDto,
): AgentAvailabilityMessageKey {
  switch (agent.availability) {
    case 'available':
      return 'agents.availability.available'
    case 'unavailable':
      return 'agents.availability.unavailable'
    case 'auth_required':
      return 'agents.availability.authRequired'
    case 'error':
      return 'agents.availability.error'
  }
}

export function agentDotStatus(
  agent: AgentRuntimeSummaryDto,
): 'idle' | 'running' | 'waiting' | 'success' | 'error' {
  const activity = agentPresentedActivity(agent)
  if (activity === 'running' || activity === 'starting' || activity === 'stopping') return 'running'
  if (activity === 'waiting_approval') return 'waiting'
  if (activity === 'unknown') return 'idle'
  if (activity === 'error') return 'error'
  if (agent.availability === 'error') return 'error'
  if (agent.availability === 'auth_required') return 'waiting'
  if (agent.availability === 'unavailable' || activity === 'offline') return 'idle'
  return 'success'
}

export function activeAgentCounts(agent: AgentRuntimeSummaryDto): AgentCountEntry[] {
  const windows = agentWindowCount(agent)
  const entries: AgentCountEntry[] = [{ key: agentPresenceCountKey(agent), count: windows.count, ...(windows.quality === 'exact' ? {} : { quality: windows.quality }) }]
  if (agent.active_runs > 0) entries.push({ key: 'runs', count: agent.active_runs })
  if (agent.active_subagents > 0) entries.push({ key: 'subagents', count: agent.active_subagents })
  return entries
}
