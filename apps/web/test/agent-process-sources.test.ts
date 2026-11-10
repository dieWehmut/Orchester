import { AGENT_FLEET_FIXTURE, parseAgentFleetSnapshot, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { describe, expect, it } from 'vitest'

import { AgentFleetPanel, AgentFleetRow, AgentMetrics, agentActivityMessageKey, agentDotStatus, groupAgentFleet } from '../src/features/agent-presence'
import { useAgentFleetStore } from '../src/stores/agent-fleet'

const observedAt = '2026-11-10T05:21:04.000Z'
const available = { environment: 'windows', status: 'available', count: 0, observed_at: observedAt } as const
const unavailable = { environment: 'wsl', status: 'unavailable', count: null, observed_at: null } as const
const agent = (overrides: Partial<AgentRuntimeSummaryDto> = {}): AgentRuntimeSummaryDto => ({
  ...AGENT_FLEET_FIXTURE.agents[0]!, activity: 'unknown', active_windows: 0,
  active_sessions: 0, active_runs: 0, active_subagents: 0,
  process_sources: [available, unavailable], ...overrides,
})

describe('process source presentation', () => {
  it('does not mistake initial legacy managed zero for a complete idle scan', () => {
    const { process_sources: _sources, ...legacyAgent } = agent({ activity: 'idle', process_sources: null })
    const parsed = parseAgentFleetSnapshot({ ...AGENT_FLEET_FIXTURE, schema_version: 2, agents: [legacyAgent] })!
    expect(agentActivityMessageKey(parsed.agents[0]!)).toBe('agents.activity.unknown')
    expect(agentDotStatus(parsed.agents[0]!)).not.toBe('success')
    expect(groupAgentFleet(parsed.agents)[0]?.key).toBe('attention')
    const row = mount(AgentFleetRow, { props: { agent: parsed.agents[0]!, compact: false } })
    expect(row.get('[data-agent-activity]').text()).toBe('Status unknown')
    expect(row.get('button').attributes('aria-label')).toContain('Status unknown')
    expect(row.get('[data-active-windows]').text()).toBe('—')
    expect(row.get('button').attributes('aria-label')).not.toContain('0 windows')
  })

  it('does not present initial v3 managed zero as a total when sources fail', () => {
    const runtime = agent({ process_sources: [{ ...unavailable, environment: 'windows' }, unavailable] })
    const metrics = mount(AgentMetrics, { props: { agent: runtime, variant: 'compact' } })
    expect(metrics.get('[data-active-windows]').text()).toBe('—')
    expect(metrics.get('[data-agent-metrics]').attributes('aria-label')).toContain('Status unknown')
    expect(metrics.get('[data-agent-metrics]').attributes('aria-label')).not.toContain('0 windows')
  })

  it('shows available counts as a lower bound without adding stale WSL counts', () => {
    const partial = agent({ activity: 'running', window_count_source: 'external_processes', active_windows: 3,
      process_sources: [{ ...available, count: 3 }, { ...unavailable, status: 'stale', count: 7, observed_at: observedAt }],
    })
    const wrapper = mount(AgentMetrics, { props: { agent: partial, variant: 'detail' } })
    expect(wrapper.get('[data-active-windows]').text()).toBe('At least 3')
    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('At least 3 processes')
    expect(wrapper.get('[data-agent-process-sources]').text()).toContain('Last observed: 7')
    const panel = mount(AgentFleetPanel, { props: { snapshot: { ...AGENT_FLEET_FIXTURE, agents: [partial] }, status: 'ready' } })
    expect(panel.get('[data-agent-group-windows]').text()).toContain('At least 3')
  })

  it('shows unknown external zero and exact zero only after every source succeeds', () => {
    const partial = agent({ window_count_source: 'external_processes' })
    const wrapper = mount(AgentMetrics, { props: { agent: partial, variant: 'detail' } })
    expect(wrapper.get('[data-active-windows]').text()).toBe('—')
    expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toContain('Status unknown')
    const exact = mount(AgentMetrics, { props: { agent: agent({ activity: 'idle', window_count_source: 'external_processes', process_sources: [available] }) } })
    expect(exact.get('[data-active-windows]').text()).toBe('0')
  })

  it('preserves managed runtime and approval while qualifying aggregate totals', () => {
    const managed = agent({ activity: 'waiting_approval', active_windows: 2, active_runs: 1 })
    expect(agentActivityMessageKey(managed)).toBe('agents.activity.waitingApproval')
    const wrapper = mount(AgentMetrics, { props: { agent: managed } })
    expect(wrapper.get('[data-active-windows]').text()).toBe('2')
    setActivePinia(createPinia())
    const store = useAgentFleetStore()
    store.applySnapshot({ ...AGENT_FLEET_FIXTURE, agents: [managed] })
    expect(store.activeWindowCountQuality).toBe('lower_bound')
    store.applySnapshot({ ...AGENT_FLEET_FIXTURE, sequence: 13, agents: [agent()] })
    expect(store.activeWindowCount).toBeNull()
    expect(store.activeWindowCountQuality).toBe('unknown')
  })

  it('keeps quality and count from the same newer snapshot when metadata alone changes', () => {
    setActivePinia(createPinia())
    const store = useAgentFleetStore()
    const runtime = agent({ activity: 'running', window_count_source: 'external_processes', active_windows: 3, process_sources: [{ ...available, count: 3 }] })
    const full = { ...AGENT_FLEET_FIXTURE, sequence: 30, agents: [runtime] }
    store.applySnapshot(full)
    expect(store.activeWindowCountQuality).toBe('exact')
    store.applySnapshot({ ...full, sequence: 31, agents: [{ ...runtime, process_sources: [{ ...available, count: 3 }, unavailable] }] })
    expect(store.activeWindowCount).toBe(3)
    expect(store.activeWindowCountQuality).toBe('lower_bound')
    store.applySnapshot(full)
    expect(store.snapshot?.sequence).toBe(31)
    expect(store.activeWindowCountQuality).toBe('lower_bound')
  })

  it('never treats failed sources as zero and preserves provider availability errors', () => {
    const failed = agent({ window_count_source: 'external_processes', process_sources: [{ ...unavailable, environment: 'windows' }, unavailable] })
    expect(groupAgentFleet([failed])[0]?.activeWindows).toBeNull()
    expect(agentActivityMessageKey(failed)).toBe('agents.activity.unknown')
    const error = { ...failed, availability: 'error' as const }
    expect(agentActivityMessageKey(error)).toBe('agents.activity.unknown')
    const offlineError = { ...error, activity: 'error' as const }
    expect(agentDotStatus(offlineError)).toBe('error')
  })

  it('keeps actual WSL running and unknown states visible alongside unavailable CLI status', () => {
    for (const activity of ['running', 'unknown'] as const) {
      const runtime = agent({ activity, availability: 'unavailable', active_windows: activity === 'running' ? 3 : 0,
        window_count_source: 'external_processes', process_sources: activity === 'running' ? [{ environment: 'wsl', status: 'available', count: 3, observed_at: observedAt }] : [unavailable],
      })
      const row = mount(AgentFleetRow, { props: { agent: runtime, compact: true } })
      expect(row.get('[data-agent-activity]').text()).toBe(activity === 'running' ? 'Service running' : 'Status unknown')
      expect(row.get('[data-agent-availability]').text()).toBe('Unavailable')
      expect(row.get('button').attributes('aria-label')).toContain('Unavailable')
    }
  })
})
