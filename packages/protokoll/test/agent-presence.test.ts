import { describe, expect, it } from 'vitest'
import {
  AGENT_EXTERNAL_PROCESS_FIXTURE,
  agentFleetCountKey,
  agentHasManagedActivity,
  agentPresenceCountKey,
  agentPresentedActivity,
  agentWindowCount,
  type AgentRuntimeSummaryDto,
} from '../src/index'

function runtime(overrides: Partial<AgentRuntimeSummaryDto> = {}): AgentRuntimeSummaryDto {
  return { ...AGENT_EXTERNAL_PROCESS_FIXTURE.agents[0]!, active_sessions: 0, active_runs: 0, active_subagents: 0, ...overrides }
}
describe('shared presence semantics', () => {
  it('keeps service and CLI counts distinct using explicit evidence rather than brand names', () => {
    const service = runtime({ provider: 'custom-backend', capabilities: ['process_presence', 'service_presence'] })
    const cli = runtime({ provider: 'openclaw', capabilities: ['process_presence'] })
    expect(agentPresenceCountKey(service)).toBe('services')
    expect(agentPresenceCountKey(cli)).toBe('processes')
    expect(agentFleetCountKey([service, cli])).toBe('instances')
  })
  it('does not convert missing observations into zero or task activity', () => {
    const service = runtime({ activity: 'offline', active_windows: 0, process_sources: null, capabilities: ['process_presence', 'service_presence'] })
    expect(agentPresentedActivity(service)).toBe('unknown')
    expect(agentWindowCount(service)).toEqual({ count: null, quality: 'unknown' })
    expect(agentHasManagedActivity(service)).toBe(false)
  })
  it('keeps confirmed current process presence as a lower bound when another source is missing', () => {
    const agent = runtime({ active_windows: 1, process_sources: [
      { environment: 'local', status: 'available', count: 1, observed_at: '2026-11-11T07:00:00.000Z' },
      { environment: 'windows', status: 'unavailable', count: null, observed_at: null },
    ] })
    expect(agentWindowCount(agent)).toEqual({ count: 1, quality: 'lower_bound' })
    expect(agentHasManagedActivity(agent)).toBe(false)
  })
})
