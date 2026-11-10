import {
  AGENT_EXTERNAL_PROCESS_FIXTURE,
  AGENT_FLEET_FIXTURE,
  type AgentRuntimeSummaryDto,
} from '@orchester/protokoll'
import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { nextTick } from 'vue'

import { AgentDetails, AgentFleetPanel, AgentFleetRow, AgentMetrics } from '../src/features/agent-presence'
import { createI18n, type I18n } from '../src/i18n'

enableAutoUnmount(afterEach)

const observedAt = '2026-11-10T08:10:00.000Z'
const copy = [
  {
    locale: 'en', process: 'process', processes: 'processes', window: 'window', windows: 'windows',
    instance: 'instance', instances: 'instances', lowerBound: 'At least 3', unknown: 'Status unknown',
    serviceActivity: 'Service running', managedActivity: 'Running', unavailable: 'Unavailable',
    sourceLabel: 'Count source', externalSource: 'External processes',
    managedSource: 'Managed sessions', desktopSource: 'Desktop windows',
    monitorOnly: 'Process monitoring only; service presence does not indicate task activity.',
    unprobedStatus: /Sign in required|Unavailable|Not installed|Not configured|Not authenticated/,
  },
  {
    locale: 'zh-CN', process: '进程', processes: '进程', window: '窗口', windows: '窗口',
    instance: '实例', instances: '实例', lowerBound: '至少 3', unknown: '状态未知',
    serviceActivity: '服务运行', managedActivity: '运行中', unavailable: '不可用',
    sourceLabel: '计数来源', externalSource: '外部进程',
    managedSource: '托管会话', desktopSource: '桌面窗口',
    monitorOnly: '仅监控进程；服务存在不代表正在执行任务。',
    unprobedStatus: /需要登录|不可用|未安装|未配置|未认证/,
  },
  {
    locale: 'zh-TW', process: '程序', processes: '程序', window: '視窗', windows: '視窗',
    instance: '實例', instances: '實例', lowerBound: '至少 3', unknown: '狀態未知',
    serviceActivity: '服務執行', managedActivity: '執行中', unavailable: '無法使用',
    sourceLabel: '計數來源', externalSource: '外部程序',
    managedSource: '受管工作階段', desktopSource: '桌面視窗',
    monitorOnly: '僅監控程序；服務存在不代表正在執行任務。',
    unprobedStatus: /需要登入|無法使用|未安裝|未設定|未認證/,
  },
] as const

function processMonitor(count: number, overrides: Partial<AgentRuntimeSummaryDto> = {}): AgentRuntimeSummaryDto {
  return {
    ...AGENT_EXTERNAL_PROCESS_FIXTURE.agents[0]!,
    agent_id: 'process-monitor',
    provider: 'gemini',
    display_name: 'Gemini CLI',
    icon_key: 'gemini',
    activity: count > 0 ? 'running' : 'offline',
    availability: 'unavailable',
    installed: false,
    configured: false,
    authenticated: false,
    active_windows: count,
    active_sessions: 0,
    active_runs: 0,
    active_subagents: 0,
    process_sources: [{ environment: 'local', status: 'available', count, observed_at: observedAt }],
    capabilities: ['monitor_only', 'process_presence'],
    ...overrides,
  }
}

function windowRuntime(source: 'managed_sessions' | 'tauri_windows', count: number): AgentRuntimeSummaryDto {
  return {
    ...AGENT_FLEET_FIXTURE.agents[0]!,
    agent_id: source,
    window_count_source: source,
    active_windows: count,
    active_sessions: 0,
    active_runs: 0,
    active_subagents: 0,
    activity: count > 0 ? 'running' : 'starting',
  }
}

function mountViews(agent: AgentRuntimeSummaryDto, i18n: I18n) {
  const global = { plugins: [i18n] }
  return [
    mount(AgentFleetRow, { props: { agent, compact: false }, global }),
    mount(AgentMetrics, { props: { agent, variant: 'detail' }, global }),
    mount(AgentDetails, { props: { agent }, global }),
  ]
}

function mountFleet(agents: AgentRuntimeSummaryDto[], i18n: I18n) {
  return mount(AgentFleetPanel, {
    props: { snapshot: { ...AGENT_FLEET_FIXTURE, agents }, status: 'ready' },
    global: { plugins: [i18n] },
  })
}

describe.each(copy)('agent count units in $locale DOM', (text) => {
  it.each([1, 3])('renders %i external processes with the right visible and accessible unit', (count) => {
    const views = mountViews(processMonitor(count), createI18n(text.locale))
    const label = count === 1 ? text.process : text.processes

    for (const wrapper of views) {
      const primary = wrapper.get('[data-agent-count-unit="processes"]')
      expect(primary.get('dt').isVisible()).toBe(true)
      expect(primary.get('dt').text()).toBe(label)
      expect(primary.get('[data-active-processes]').text()).toBe(String(count))
      expect(primary.get('[data-active-windows]').text()).toBe(String(count))
      expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toBe(`${count} ${label}`)
      expect(wrapper.find('[data-agent-count-unit="windows"]').exists()).toBe(false)
    }
    expect(views[0]!.get('button').attributes('aria-label')).toContain(`${count} ${label}`)
    for (const wrapper of views.slice(1)) {
      expect(wrapper.get('[data-agent-detail="processes"] dt').text()).toBe(label)
      expect(wrapper.get('[data-agent-window-source]').text()).toBe(`${text.sourceLabel}: ${text.externalSource}`)
    }
  })

  it('keeps current positive process evidence as a lower bound in rows, details and groups', () => {
    const runtime = processMonitor(3, {
      process_sources: [
        { environment: 'local', status: 'available', count: 3, observed_at: observedAt },
        { environment: 'wsl', status: 'stale', count: 7, observed_at: observedAt },
      ],
    })
    const i18n = createI18n(text.locale)
    const views = mountViews(runtime, i18n)
    const summary = `${text.lowerBound} ${text.processes}`

    for (const wrapper of views) {
      expect(wrapper.get('[data-agent-count-unit="processes"] dt').text()).toBe(text.processes)
      expect(wrapper.get('[data-active-processes]').text()).toBe(text.lowerBound)
      expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toBe(summary)
    }
    expect(views[0]!.get('button').attributes('aria-label')).toContain(summary)
    const badge = mountFleet([runtime], i18n).get('[data-agent-group-windows]')
    expect(badge.attributes('data-agent-count-unit')).toBe('processes')
    expect(badge.text()).toBe(summary)
    expect(badge.attributes('aria-label')).toBe(summary)
  })

  it('renders unknown process counts as a dash and announces unknown processes', () => {
    const runtime = processMonitor(0, {
      activity: 'unknown',
      process_sources: [{ environment: 'local', status: 'stale', count: 7, observed_at: observedAt }],
    })
    const i18n = createI18n(text.locale)
    const views = mountViews(runtime, i18n)
    const summary = `${text.unknown} ${text.processes}`

    for (const wrapper of views) {
      expect(wrapper.get('[data-agent-count-unit="processes"] dt').text()).toBe(text.processes)
      expect(wrapper.get('[data-active-processes]').text()).toBe('—')
      expect(wrapper.get('[data-active-windows]').text()).toBe('—')
      expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toBe(summary)
    }
    expect(views[0]!.get('button').attributes('aria-label')).toContain(summary)
    const badge = mountFleet([runtime], i18n).get('[data-agent-group-windows]')
    expect(badge.attributes('data-agent-count-unit')).toBe('processes')
    expect(badge.text()).toBe(`— ${text.processes}`)
    expect(badge.attributes('aria-label')).toBe(summary)
  })

  it.each(['managed_sessions', 'tauri_windows'] as const)('preserves window units for %s', (source) => {
    for (const count of [1, 3]) {
      const views = mountViews(windowRuntime(source, count), createI18n(text.locale))
      const label = count === 1 ? text.window : text.windows
      for (const wrapper of views) {
        const primary = wrapper.get('[data-agent-count-unit="windows"]')
        expect(primary.get('dt').isVisible()).toBe(true)
        expect(primary.get('dt').text()).toBe(label)
        expect(primary.get('[data-active-windows]').text()).toBe(String(count))
        expect(wrapper.find('[data-active-processes]').exists()).toBe(false)
        expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toBe(`${count} ${label}`)
      }
      expect(views[0]!.get('button').attributes('aria-label')).toContain(`${count} ${label}`)
      for (const wrapper of views.slice(1)) {
        expect(wrapper.get('[data-agent-detail="windows"] dt').text()).toBe(label)
        expect(wrapper.get('[data-agent-window-source]').text()).toBe(
          `${text.sourceLabel}: ${source === 'managed_sessions' ? text.managedSource : text.desktopSource}`,
        )
      }
    }
  })

  it.each([1, 3])('uses process units for a homogeneous group totaling %i', (count) => {
    const agents = count === 1
      ? [processMonitor(1)]
      : [processMonitor(1), processMonitor(2, { agent_id: 'second-process-monitor' })]
    const panel = mountFleet(agents, createI18n(text.locale))
    const label = count === 1 ? text.process : text.processes
    const badge = panel.get('[data-agent-group="active"] [data-agent-group-windows]')

    expect(badge.attributes('data-agent-count-unit')).toBe('processes')
    expect(badge.isVisible()).toBe(true)
    expect(badge.text()).toBe(`${count} ${label}`)
    expect(badge.attributes('aria-label')).toBe(`${count} ${label}`)
    expect(panel.get('[data-agent-id="process-monitor"] [data-agent-count-unit="processes"] dt').text()).toBe(text.process)
  })

  it('aggregates managed sessions and desktop windows with the same window unit', () => {
    const panel = mountFleet([windowRuntime('managed_sessions', 1), windowRuntime('tauri_windows', 2)], createI18n(text.locale))
    const badge = panel.get('[data-agent-group="active"] [data-agent-group-windows]')

    expect(badge.attributes('data-agent-count-unit')).toBe('windows')
    expect(badge.text()).toBe(`3 ${text.windows}`)
    expect(badge.attributes('aria-label')).toBe(`3 ${text.windows}`)
    expect(panel.get('[data-agent-id="managed_sessions"] [data-agent-count-unit="windows"] dt').text()).toBe(text.window)
    expect(panel.get('[data-agent-id="tauri_windows"] [data-agent-count-unit="windows"] dt').text()).toBe(text.windows)
  })

  it.each([0, 2])('uses instance units for mixed process and window sources with %i windows', (windows) => {
    const panel = mountFleet([processMonitor(1), windowRuntime('managed_sessions', windows)], createI18n(text.locale))
    const badge = panel.get('[data-agent-group="active"] [data-agent-group-windows]')
    const total = windows + 1
    const label = total === 1 ? text.instance : text.instances

    expect(badge.attributes('data-agent-count-unit')).toBe('instances')
    expect(badge.isVisible()).toBe(true)
    expect(badge.text()).toBe(`${total} ${label}`)
    expect(badge.attributes('aria-label')).toBe(`${total} ${label}`)

    const processRow = panel.get('[data-agent-id="process-monitor"]')
    expect(processRow.get('[data-agent-count-unit="processes"] dt').text()).toBe(text.process)
    expect(processRow.get('button').attributes('aria-label')).toContain(`1 ${text.process}`)
    const windowRow = panel.get('[data-agent-id="managed_sessions"]')
    expect(windowRow.get('[data-agent-count-unit="windows"] dt').text()).toBe(text.windows)
    expect(windowRow.get('button').attributes('aria-label')).toContain(`${windows} ${text.windows}`)
    expect(panel.find('[data-agent-count][data-agent-count-unit="instances"]').exists()).toBe(false)
  })

  it('uses service activity for a legacy external runtime while preserving CLI availability', () => {
    const agent: AgentRuntimeSummaryDto = {
      ...AGENT_EXTERNAL_PROCESS_FIXTURE.agents[0]!,
      availability: 'unavailable',
      active_sessions: 0,
      active_runs: 0,
      active_subagents: 0,
      capabilities: ['streaming'],
    }
    const row = mount(AgentFleetRow, { props: { agent }, global: { plugins: [createI18n(text.locale)] } })

    expect(row.get('[data-agent-activity]').text()).toBe(text.serviceActivity)
    expect(row.get('[data-agent-availability]').text()).toBe(text.unavailable)
    expect(row.get('button').attributes('aria-label')).toContain(text.serviceActivity)
    expect(row.get('button').attributes('aria-label')).toContain(text.unavailable)
    expect(row.get('button').attributes('aria-label')).toContain(`3 ${text.processes}`)
  })

  it('keeps actual managed work as running even when external processes supply the primary count', () => {
    const agent: AgentRuntimeSummaryDto = {
      ...AGENT_EXTERNAL_PROCESS_FIXTURE.agents[0]!,
      availability: 'unavailable',
      active_sessions: 1,
      active_runs: 1,
      active_subagents: 0,
      capabilities: ['streaming'],
    }
    const row = mount(AgentFleetRow, { props: { agent }, global: { plugins: [createI18n(text.locale)] } })

    expect(row.get('[data-agent-activity]').text()).toBe(text.managedActivity)
    expect(row.get('[data-active-runs]').text()).toBe('1')
    expect(row.get('button').attributes('aria-label')).toContain(text.managedActivity)
    expect(row.get('button').attributes('aria-label')).not.toContain(text.serviceActivity)
    expect(row.get('button').attributes('aria-label')).toContain(`3 ${text.processes}`)
  })

  it.each(['unavailable', 'auth_required'] as const)('keeps %s adapter status hidden for a process monitor', (availability) => {
    const [row, metrics, details] = mountViews(processMonitor(0, { availability }), createI18n(text.locale))

    expect(row!.find('[data-agent-availability]').exists()).toBe(false)
    expect(details!.find('[data-agent-details-availability]').exists()).toBe(false)
    expect(details!.find('[data-agent-capabilities]').exists()).toBe(false)
    expect(details!.get('[data-agent-monitor-only]').text()).toBe(text.monitorOnly)
    expect(metrics!.get('[data-active-processes]').text()).toBe('0')
    for (const wrapper of [row!, details!]) expect(wrapper.text()).not.toMatch(text.unprobedStatus)
    expect(row!.get('button').attributes('aria-label')).not.toMatch(text.unprobedStatus)
  })
})

it('updates process and mixed-group labels in the mounted DOM when the locale changes', async () => {
  const i18n = createI18n('en')
  const views = mountViews(processMonitor(1), i18n)
  const panel = mountFleet([processMonitor(1), windowRuntime('tauri_windows', 2)], i18n)

  for (const text of copy) {
    i18n.setLocale(text.locale)
    await nextTick()
    for (const wrapper of views) {
      expect(wrapper.get('[data-agent-count-unit="processes"] dt').text()).toBe(text.process)
      expect(wrapper.get('[data-agent-metrics]').attributes('aria-label')).toBe(`1 ${text.process}`)
    }
    expect(views[0]!.get('button').attributes('aria-label')).toContain(`1 ${text.process}`)
    expect(views[2]!.get('[data-agent-window-source]').text()).toBe(`${text.sourceLabel}: ${text.externalSource}`)
    const badge = panel.get('[data-agent-group-windows]')
    expect(badge.text()).toBe(`3 ${text.instances}`)
    expect(badge.attributes('aria-label')).toBe(`3 ${text.instances}`)
    expect(panel.get('[data-agent-id="tauri_windows"] [data-agent-count-unit="windows"] dt').text()).toBe(text.windows)
  }
})
