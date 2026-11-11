// @vitest-environment jsdom
import { AGENT_FLEET_FIXTURE, type AgentFleetSnapshotDto } from '@orchester/protokoll'
import { createApp, effectScope } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import AgentMonitorList from '../src/components/AgentMonitorList.vue'
import { useRemoteControl } from '../src/composables/use-remote-control'
import { RemoteClient, RemoteError } from '../src/client'
import { credentials } from '../src/credentials'

const observedAt = '2026-11-11T07:00:00.000Z'
function snapshot(sequence = 1): AgentFleetSnapshotDto {
  return {
    schema_version: 3, sequence, generated_at: observedAt,
    agents: ['workbuddy', 'qwen', 'openclaw'].map((provider) => ({
      ...AGENT_FLEET_FIXTURE.agents[0]!, agent_id: provider, provider,
      display_name: provider === 'qwen' ? 'Qwen Code' : provider === 'workbuddy' ? 'WorkBuddy' : 'OpenClaw',
      icon_key: provider, availability: 'unavailable', activity: 'running',
      installed: false, configured: false, authenticated: false,
      active_windows: 1, active_sessions: 0, active_runs: 0, active_subagents: 0,
      window_count_source: 'external_processes',
      process_sources: [{ environment: 'local', status: 'available', count: 1, observed_at: observedAt }],
      capabilities: ['monitor_only', 'process_presence', ...(provider === 'qwen' ? [] : ['service_presence'])],
    })),
  }
}
let scope: ReturnType<typeof effectScope>
let control: ReturnType<typeof useRemoteControl>
let request: ReturnType<typeof vi.spyOn>
const mounted: { app: ReturnType<typeof createApp>; element: HTMLElement }[] = []
function mountMonitor(props: { snapshot: AgentFleetSnapshotDto | null; status: 'ready' | 'stale'; error: string }) {
  const element = document.createElement('div')
  document.body.append(element)
  const app = createApp(AgentMonitorList, props)
  app.mount(element)
  mounted.push({ app, element })
  return element
}
function node(root: Element, selector: string): HTMLElement {
  const result = root.querySelector<HTMLElement>(selector)
  if (!result) throw new Error(`Missing monitor element: ${selector}`)
  return result
}
beforeEach(async () => {
  vi.spyOn(credentials, 'get').mockResolvedValue({ value: 'test-credential' })
  request = vi.spyOn(RemoteClient.prototype, 'request').mockImplementation(async (path) => {
    if (path === '/info') return { name: 'test computer' }
    if (path === '/agents') return { agents: [] }
    return { runs: [] }
  })
  scope = effectScope()
  control = scope.run(() => useRemoteControl())!
  control.address.value = 'localhost'
  await control.connect()
})
afterEach(() => {
  for (const { app, element } of mounted.splice(0)) { app.unmount(); element.remove() }
  scope.stop()
  vi.restoreAllMocks()
  localStorage.clear()
})

it('loads the authorized typed status endpoint and renders official identities without launch controls', async () => {
  request.mockResolvedValueOnce(snapshot())
  await control.refreshAgentStatus()
  expect(request).toHaveBeenLastCalledWith('/agents/status')
  expect(control.agentSnapshotStatus.value).toBe('ready')
  const wrapper = mountMonitor({ snapshot: control.agentSnapshot.value, status: 'ready', error: '' })
  for (const provider of ['workbuddy', 'qwen', 'openclaw']) {
    const row = node(wrapper, `[data-mobile-agent-id="${provider}"]`)
    expect(node(row, `[data-agent-icon="${provider}"] img`).getAttribute('src')).toBeTruthy()
    expect(node(row, '[data-mobile-agent-activity]').textContent).toBe('服务运行')
    expect(node(row, '[data-mobile-agent-count]').textContent).toBe('1')
    expect(node(row, '[data-mobile-agent-count-unit]').getAttribute('data-mobile-agent-count-unit')).toBe(provider === 'qwen' ? 'processes' : 'services')
    expect(row.textContent).not.toMatch(/未安装|未配置|未认证|可用|任务数量/)
  }
  expect(wrapper.querySelector('button')).toBeNull()
})

it('retains the last list after failure while masking cached counts and activity as unknown', async () => {
  request.mockResolvedValueOnce(snapshot())
  await control.refreshAgentStatus()
  request.mockRejectedValueOnce(new Error('offline'))
  await control.refreshAgentStatus()
  expect(control.agentSnapshot.value?.agents).toHaveLength(3)
  expect(control.agentSnapshotStatus.value).toBe('stale')
  const wrapper = mountMonitor({ snapshot: control.agentSnapshot.value, status: 'stale', error: control.agentSnapshotError.value })
  expect([...wrapper.querySelectorAll('[data-mobile-agent-count]')].map((node) => node.textContent)).toEqual(['—', '—', '—'])
  expect([...wrapper.querySelectorAll('[data-mobile-agent-activity]')].every((node) => node.textContent === '状态未知')).toBe(true)
})

it('preserves service lower bounds and only announces zero for complete observations', () => {
  const incoming = snapshot()
  incoming.agents[0]!.process_sources!.push({ environment: 'wsl', status: 'stale', count: 5, observed_at: observedAt })
  incoming.agents[1] = { ...incoming.agents[1]!, active_windows: 0, activity: 'offline', process_sources: [{ environment: 'local', status: 'available', count: 0, observed_at: observedAt }] }
  incoming.agents[2] = { ...incoming.agents[2]!, active_windows: 0, activity: 'offline', process_sources: null }
  const wrapper = mountMonitor({ snapshot: incoming, status: 'ready', error: '' })
  expect(node(wrapper, '[data-mobile-agent-id="workbuddy"] [data-mobile-agent-count]').textContent).toBe('至少 1')
  expect(node(wrapper, '[data-mobile-agent-id="qwen"] [data-mobile-agent-activity]').textContent).toBe('未发现运行进程')
  expect(node(wrapper, '[data-mobile-agent-id="qwen"] [data-mobile-agent-count]').textContent).toBe('0')
  expect(node(wrapper, '[data-mobile-agent-id="openclaw"] [data-mobile-agent-count]').textContent).toBe('—')
  expect(node(wrapper, '[data-mobile-agent-id="openclaw"] [data-mobile-agent-activity]').textContent).toBe('状态未知')
})

it('rejects malformed server payloads and stops future status reads after expired authorization', async () => {
  request.mockResolvedValueOnce({ ...snapshot(), schema_version: 99 })
  await control.refreshAgentStatus()
  expect(control.agentSnapshot.value).toBeNull()
  expect(control.agentSnapshotStatus.value).toBe('error')
  request.mockRejectedValueOnce(new RemoteError(401, 'expired'))
  await control.refreshAgentStatus()
  expect(control.connectionStatus.value).toBe('needs_pairing')
  const calls = request.mock.calls.length
  await control.refreshAgentStatus()
  expect(request.mock.calls).toHaveLength(calls)
})

it('drops an old response after switching computers and accepts a new server sequence after restart', async () => {
  let resolve!: (value: unknown) => void
  request.mockReturnValueOnce(new Promise((r) => { resolve = r }))
  const old = control.refreshAgentStatus()
  control.address.value = 'second'
  await control.connect()
  resolve(snapshot(20))
  await old
  expect(control.agentSnapshot.value).toBeNull()
  request.mockResolvedValueOnce(snapshot(20))
  await control.refreshAgentStatus()
  request.mockResolvedValueOnce(snapshot(1))
  await control.refreshAgentStatus()
  expect(control.agentSnapshot.value?.sequence).toBe(1)
  expect(control.agentSnapshotStatus.value).toBe('ready')
})
