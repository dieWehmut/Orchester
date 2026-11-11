import { agentIdentity } from '@orchester/design/agent-identities'
import { AGENT_FLEET_FIXTURE, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { AgentDetails, AgentFleetRow, agentActivityMessageKey, agentPresenceGroupKey } from '../src/features/agent-presence'
import { createI18n } from '../src/i18n'

function monitor(overrides: Partial<AgentRuntimeSummaryDto> = {}): AgentRuntimeSummaryDto {
  return {
    ...AGENT_FLEET_FIXTURE.agents[0]!,
    agent_id: 'gemini-monitor',
    provider: 'gemini',
    display_name: 'Gemini',
    icon_key: 'gemini',
    availability: 'unavailable',
    activity: 'offline',
    installed: false,
    configured: false,
    authenticated: false,
    active_windows: 0,
    active_sessions: 0,
    active_runs: 0,
    active_subagents: 0,
    window_count_source: 'external_processes',
    process_sources: [{ environment: 'local', status: 'available', count: 0, observed_at: '2026-11-10T08:10:00.000Z' }],
    capabilities: ['monitor_only', 'process_presence'],
    ...overrides,
  }
}

describe('agent process monitors', () => {
  it.each([
    { provider: 'gemini', display_name: 'Gemini CLI', label: 'Google', dark: 'gemini.png', light: 'gemini.png' },
    { provider: 'antigravity', display_name: 'Antigravity', label: 'Google', dark: 'antigravity.svg', light: 'antigravity.svg' },
    { provider: 'cursor', display_name: 'Cursor', label: 'Cursor', dark: 'cursor-dark.svg', light: 'cursor-light.svg' },
    { provider: 'deepseek', display_name: 'DeepSeek Harness', label: 'DeepSeek', dark: 'deepseek-harness-dark.svg', light: 'deepseek-harness-light.svg' },
    { provider: 'workbuddy', display_name: 'WorkBuddy', label: 'Tencent', dark: 'workbuddy.svg', light: 'workbuddy.svg' },
    { provider: 'qwen', display_name: 'Qwen Code', label: 'Qwen', dark: 'qwen.svg', light: 'qwen.svg' },
    { provider: 'openclaw', display_name: 'OpenClaw', label: 'OpenClaw', dark: 'openclaw.svg', light: 'openclaw.svg' },
  ])('renders the $display_name identity and local official marks in row and details', ({ provider, display_name, label, dark, light }) => {
    const agent = monitor({ provider, display_name, icon_key: 'custom-provider' })
    const row = mount(AgentFleetRow, { props: { agent } })
    const details = mount(AgentDetails, { props: { agent } })

    for (const wrapper of [row, details]) {
      expect(wrapper.get('[data-agent-provider-label]').text()).toBe(label)
      expect(wrapper.get(`[data-agent-provider="${provider}"]`)).toBeTruthy()
      expect(wrapper.get(`[data-agent-icon="${provider}"] .agent-icon__dark`).attributes('src')).toBe(agentIdentity(provider)!.marks.dark)
      expect(wrapper.get(`[data-agent-icon="${provider}"] .agent-icon__light`).attributes('src')).toBe(agentIdentity(provider)!.marks.light)
    }
    expect(row.get('button').attributes('aria-label')).toContain(display_name)
    expect(details.get('[data-agent-details-name]').text()).toBe(display_name)
    expect(existsSync(resolve('../../packages/design/src/assets/agents', dark))).toBe(true)
    expect(existsSync(resolve('../../packages/design/src/assets/agents', light))).toBe(true)
  })

  it('renders complete zero observations as no running process and hides adapter availability', () => {
    const agent = monitor()
    const wrapper = mount(AgentFleetRow, { props: { agent } })

    expect(wrapper.get('[data-agent-activity]').text()).toBe('No running process found')
    expect(wrapper.get('[data-active-windows]').text()).toBe('0')
    expect(wrapper.find('[data-agent-availability]').exists()).toBe(false)
    expect(wrapper.get('button').attributes('aria-label')).not.toContain('Unavailable')
    expect(agentPresenceGroupKey(agent)).toBe('attention')
  })

  it('renders current process presence as a service without implying task work', () => {
    const agent = monitor({
      activity: 'running',
      active_windows: 2,
      process_sources: [{ environment: 'local', status: 'available', count: 2, observed_at: '2026-11-10T08:10:00.000Z' }],
    })
    const wrapper = mount(AgentFleetRow, { props: { agent } })

    expect(wrapper.get('[data-agent-activity]').text()).toBe('Service running')
    expect(wrapper.get('[data-active-windows]').text()).toBe('2')
    expect(wrapper.find('[data-active-runs]').exists()).toBe(false)
    expect(wrapper.find('.status-dot--pulse').exists()).toBe(false)
    expect(agentPresenceGroupKey(agent)).toBe('active')
  })

  it('preserves current positive evidence as a lower bound when another source is unavailable', () => {
    const agent = monitor({
      activity: 'running',
      active_windows: 1,
      process_sources: [
        { environment: 'local', status: 'available', count: 1, observed_at: '2026-11-10T08:10:00.000Z' },
        { environment: 'windows', status: 'unavailable', count: null, observed_at: null },
      ],
    })
    const wrapper = mount(AgentFleetRow, { props: { agent } })

    expect(wrapper.get('[data-agent-activity]').text()).toBe('Service running')
    expect(wrapper.get('[data-active-windows]').text()).toBe('At least 1')
  })

  it.each([
    null,
    [{ environment: 'local' as const, status: 'unavailable' as const, count: null, observed_at: null }],
    [{ environment: 'local' as const, status: 'stale' as const, count: 3, observed_at: '2026-11-10T08:09:00.000Z' }],
  ])('renders absent current process evidence as unknown rather than zero: %j', (process_sources) => {
    const agent = monitor({ activity: 'unknown', process_sources })
    const wrapper = mount(AgentFleetRow, { props: { agent } })

    expect(wrapper.get('[data-agent-activity]').text()).toBe('Status unknown')
    expect(wrapper.get('[data-active-windows]').text()).toBe('—')
    expect(wrapper.get('button').attributes('aria-label')).toContain('Status unknown processes')
  })

  it('explains monitoring in details without unprobed installation or authentication assertions', () => {
    const wrapper = mount(AgentDetails, {
      props: { agent: monitor() },
      global: { plugins: [createI18n('zh-CN')] },
    })

    expect(wrapper.get('[data-agent-details-activity]').text()).toBe('未发现运行进程')
    expect(wrapper.get('[data-agent-monitor-only]').text()).toBe('仅监控进程；服务存在不代表正在执行任务。')
    expect(wrapper.find('[data-agent-details-availability]').exists()).toBe(false)
    expect(wrapper.find('[data-agent-capabilities]').exists()).toBe(false)
    expect(wrapper.text()).not.toMatch(/未安装|未配置|未认证|需要登录|不可用|false/)
    expect(wrapper.get('[data-process-source-status="available"]').text()).toContain('当前：0')
  })

  it('preserves managed work state and adapter details for a registered provider', () => {
    const agent = monitor({
      availability: 'available',
      activity: 'running',
      active_windows: 1,
      active_runs: 1,
      active_sessions: 1,
      window_count_source: 'managed_sessions',
      capabilities: ['streaming', 'process_presence'],
    })
    const wrapper = mount(AgentDetails, { props: { agent } })

    expect(wrapper.get('[data-agent-details-activity]').text()).toBe('Running')
    expect(wrapper.get('[data-agent-details-availability]').text()).toContain('Available')
    expect(wrapper.get('[data-agent-capabilities]').text()).toContain('streaming')
    expect(wrapper.find('[data-agent-monitor-only]').exists()).toBe(false)
    expect(wrapper.find('.status-dot--pulse').exists()).toBe(true)
    expect(agentActivityMessageKey({ ...agent, activity: 'waiting_approval' })).toBe('agents.activity.waitingApproval')
  })

  it('uses service copy for a registered process monitor without active managed work', () => {
    const agent = monitor({ activity: 'running', active_windows: 1, capabilities: ['streaming', 'process_presence'] })

    expect(agentActivityMessageKey(agent)).toBe('agents.activity.serviceRunning')
  })
})
