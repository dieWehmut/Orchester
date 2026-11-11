import { AGENT_EXTERNAL_PROCESS_FIXTURE, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { AgentDetails, AgentFleetPanel, AgentFleetRow, agentPresenceCountKey } from '../src/features/agent-presence'
import { createI18n } from '../src/i18n'

enableAutoUnmount(afterEach)
const observedAt = '2026-11-11T07:00:00.000Z'

function service(count: number, overrides: Partial<AgentRuntimeSummaryDto> = {}): AgentRuntimeSummaryDto {
  return {
    ...AGENT_EXTERNAL_PROCESS_FIXTURE.agents[0]!,
    agent_id: 'local-service', provider: 'custom-backend', display_name: 'Local service',
    active_windows: count, active_sessions: 0, active_runs: 0, active_subagents: 0,
    activity: count ? 'running' : 'offline',
    capabilities: ['monitor_only', 'process_presence', 'service_presence'],
    process_sources: [{ environment: 'local', status: 'available', count, observed_at: observedAt }],
    ...overrides,
  }
}

describe.each([
  { locale: 'en', one: 'service process', many: 'service processes', activity: 'Service running', lower: 'At least 1' },
  { locale: 'zh-CN', one: '服务进程', many: '服务进程', activity: '服务运行', lower: '至少 1' },
  { locale: 'zh-TW', one: '服務程序', many: '服務程序', activity: '服務執行', lower: '至少 1' },
])('explicit service-process counts in $locale', (text) => {
  it.each([1, 3])('uses service units for %i known services even for a custom provider', (count) => {
    const agent = service(count)
    const global = { plugins: [createI18n(text.locale)] }
    const label = count === 1 ? text.one : text.many
    const row = mount(AgentFleetRow, { props: { agent }, global })
    const details = mount(AgentDetails, { props: { agent }, global })
    for (const wrapper of [row, details]) {
      expect(wrapper.get('[data-agent-count-unit="services"] dt').text()).toBe(label)
      expect(wrapper.get('[data-active-services]').text()).toBe(String(count))
      expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toBe(`${count} ${label}`)
      expect(wrapper.find('[data-active-runs]').exists()).toBe(false)
      expect(wrapper.find('[data-active-subagents]').exists()).toBe(false)
    }
    expect(row.get('[data-agent-activity]').text()).toBe(text.activity)
    const panel = mount(AgentFleetPanel, {
      props: { snapshot: { ...AGENT_EXTERNAL_PROCESS_FIXTURE, agents: [agent] }, status: 'ready' }, global,
    })
    expect(panel.get('[data-agent-group-windows]').attributes('data-agent-count-unit')).toBe('services')
    expect(panel.get('[data-agent-group-windows]').text()).toBe(`${count} ${label}`)
  })

  it('preserves a service lower bound and uses instance totals for mixed service and CLI sources', () => {
    const agent = service(1, {
      process_sources: [
        { environment: 'local', status: 'available', count: 1, observed_at: observedAt },
        { environment: 'wsl', status: 'stale', count: 5, observed_at: observedAt },
      ],
    })
    const global = { plugins: [createI18n(text.locale)] }
    const row = mount(AgentFleetRow, { props: { agent }, global })
    expect(row.get('[data-active-services]').text()).toBe(text.lower)
    const cli = service(2, { agent_id: 'cli', capabilities: ['monitor_only', 'process_presence'] })
    const panel = mount(AgentFleetPanel, {
      props: { snapshot: { ...AGENT_EXTERNAL_PROCESS_FIXTURE, agents: [agent, cli] }, status: 'ready' }, global,
    })
    expect(panel.get('[data-agent-group-windows]').attributes('data-agent-count-unit')).toBe('instances')
    expect(panel.get('[data-agent-id="cli"] [data-agent-count-unit="processes"]')).toBeTruthy()
  })
})

it('requires an explicit service capability rather than inferring count units from the provider brand', () => {
  expect(agentPresenceCountKey(service(1, { provider: 'qwen' }))).toBe('services')
  expect(agentPresenceCountKey(service(1, { provider: 'openclaw', capabilities: ['monitor_only', 'process_presence'] }))).toBe('processes')
})
