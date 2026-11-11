import { AGENT_IDENTITIES } from '@orchester/design/agent-identities'
import {
  AGENT_FLEET_FIXTURE,
  type AgentFleetSnapshotDto,
  type AgentRuntimeSummaryDto,
} from '@orchester/protokoll'
import { enableAutoUnmount, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { createAgentsApi } from '../src/api/agents'
import type { HttpClient } from '../src/api/http'
import { AgentDetails, AgentFleetPanel, AgentFleetRow } from '../src/features/agent-presence'
import { createI18n } from '../src/i18n'
import { useAgentFleetStore } from '../src/stores/agent-fleet'

enableAutoUnmount(afterEach)

const providers = Object.values(AGENT_IDENTITIES).map((identity) => ({ provider: identity.key, displayName: identity.displayName }))
const serviceProviders = new Set(['workbuddy', 'openclaw', 'claude_app', 'trae', 'zcode', 'craft'])
const sharedEditorProviders = new Set(['cline', 'roo', 'kilo'])
const observedAt = '2026-11-11T07:00:00.000Z'

function monitor(provider: typeof providers[number], overrides: Partial<AgentRuntimeSummaryDto> = {}): AgentRuntimeSummaryDto {
  const agent: AgentRuntimeSummaryDto = {
    ...AGENT_FLEET_FIXTURE.agents[0]!,
    agent_id: provider.provider,
    provider: provider.provider,
    display_name: provider.displayName,
    icon_key: provider.provider,
    availability: 'unavailable',
    activity: 'unknown',
    installed: false,
    configured: false,
    authenticated: false,
    active_windows: 0,
    active_sessions: 0,
    active_runs: 0,
    active_subagents: 0,
    window_count_source: 'external_processes',
    process_sources: null,
    last_heartbeat_at: null,
    last_error: null,
    capabilities: ['monitor_only', 'process_presence', ...(serviceProviders.has(provider.provider) ? ['service_presence'] : []), ...(sharedEditorProviders.has(provider.provider) ? ['shared_editor_presence_unknown'] : [])],
    updated_at: observedAt,
    ...overrides,
  }
  if (sharedEditorProviders.has(provider.provider) && agent.process_sources && !agent.process_sources.some((source) => source.status === 'unavailable')) {
    agent.process_sources.push({ environment: 'local', status: 'unavailable', count: null, observed_at: null })
  }
  return agent
}

function mountViews(agent: AgentRuntimeSummaryDto) {
  const global = { plugins: [createI18n('zh-CN')] }
  return [
    mount(AgentFleetRow, { props: { agent }, global }),
    mount(AgentDetails, { props: { agent }, global }),
  ]
}

describe('new monitors through the shared status API and fleet', () => {
  it('keeps every documented server monitor visible when no adapter or observation is available', async () => {
    const incoming: AgentFleetSnapshotDto = {
      schema_version: 3,
      sequence: 1,
      generated_at: observedAt,
      agents: providers.map((provider) => monitor(provider)),
    }
    const get = vi.fn(async () => JSON.parse(JSON.stringify(incoming)) as unknown)
    const api = createAgentsApi({ get } as unknown as HttpClient)
    setActivePinia(createPinia())
    const store = useAgentFleetStore()
    store.configure(api)
    await store.load()

    expect(store.status).toBe('ready')
    expect(get).toHaveBeenCalledWith('/agents/status')
    expect(store.snapshot?.agents.map((agent) => agent.provider)).toHaveLength(20)
    expect(store.activeSubagentCount).toBe(0)
    const panel = mount(AgentFleetPanel, {
      props: { snapshot: store.snapshot, status: store.status },
      global: { plugins: [createI18n('zh-CN')] },
    })
    for (const provider of providers) {
      const row = panel.get(`[data-agent-id="${provider.provider}"]`)
      expect(row.text()).toContain(provider.displayName)
      expect(row.get('[data-agent-activity]').text()).toBe('状态未知')
      expect(row.get('[data-active-processes]').text()).toBe('—')
      expect(row.find('[data-agent-availability]').exists()).toBe(false)
      expect(row.get(`[data-agent-icon="${provider.provider}"] img`).attributes('src')).toBeTruthy()
      expect(row.find('[data-agent-icon="generic"]').exists()).toBe(false)
    }
  })
})

describe.each(providers)('$displayName monitoring semantics', (provider) => {
  it('shows confirmed process presence as service activity with process units and zero claimed tasks', () => {
    const agent = monitor(provider, {
      activity: 'running',
      active_windows: 1,
      process_sources: [{ environment: 'local', status: 'available', count: 1, observed_at: observedAt }],
    })
    const [row, details] = mountViews(agent)
    expect(row!.get('[data-agent-activity]').text()).toBe('服务运行')
    expect(row!.get(`[data-agent-count-unit="${serviceProviders.has(provider.provider) ? 'services' : 'processes'}"] dt`).text()).toBe(serviceProviders.has(provider.provider) ? '服务进程' : '进程')
    expect(row!.get('[data-active-processes]').text()).toBe(sharedEditorProviders.has(provider.provider) ? '至少 1' : '1')
    expect(details!.get('[data-agent-details-activity]').text()).toBe('服务运行')
    expect(details!.get('[data-agent-monitor-only]').text()).toBe('仅监控进程；服务存在不代表正在执行任务。')
    for (const wrapper of [row!, details!]) {
      expect(wrapper.find('[data-active-runs]').exists()).toBe(false)
      expect(wrapper.find('[data-active-subagents]').exists()).toBe(false)
      expect(wrapper.find('.status-dot--pulse').exists()).toBe(false)
      expect(wrapper.find('[data-agent-availability], [data-agent-details-availability]').exists()).toBe(false)
      expect(wrapper.text()).not.toMatch(/未安装|未配置|未认证|需要登录/)
    }
    if (sharedEditorProviders.has(provider.provider)) expect(details!.get('[data-process-source-status="unavailable"]').text()).toContain('来源不可用')
  })

  it('announces no running process only for a complete zero observation', () => {
    const [row, details] = mountViews(monitor(provider, {
      activity: 'offline',
      process_sources: [{ environment: 'local', status: 'available', count: 0, observed_at: observedAt }],
    }))
    const complete = !sharedEditorProviders.has(provider.provider)
    expect(row!.get('[data-agent-activity]').text()).toBe(complete ? '未发现运行进程' : '状态未知')
    expect(row!.get('[data-active-processes]').text()).toBe(complete ? '0' : '—')
    expect(details!.get('[data-agent-details-activity]').text()).toBe(complete ? '未发现运行进程' : '状态未知')
  })

  it('preserves positive evidence as a lower bound without adding old cached counts', () => {
    const [row, details] = mountViews(monitor(provider, {
      activity: 'running',
      active_windows: 1,
      process_sources: [
        { environment: 'local', status: 'available', count: 1, observed_at: observedAt },
        { environment: 'wsl', status: 'stale', count: 5, observed_at: observedAt },
        { environment: 'windows', status: 'unavailable', count: null, observed_at: null },
      ],
    }))
    for (const wrapper of [row!, details!]) expect(wrapper.get('[data-active-processes]').text()).toBe('至少 1')
    expect(row!.get('button').attributes('aria-label')).toContain(serviceProviders.has(provider.provider) ? '至少 1 服务进程' : '至少 1 进程')
  })
})
