import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createHttpClient } from '../src/api/http'
import { createAppStores } from '../src/stores/app'
import { createAppRouter } from '../src/router'
import AgentActivityView from '../src/features/agent-presence/components/AgentActivityView.vue'
import AgentActivitySidebar from '../src/features/agent-presence/components/AgentActivitySidebar.vue'
import { defineComponent, h, toRef, type PropType } from 'vue'
import { useAgentActivity } from '../src/features/agent-presence/use-agent-activity'
import type { ExternalActivityProvider } from '../src/api/agent-activity'
import WorkspaceView from '../src/views/WorkspaceView.vue'
import type { ActivitySummary, ActivityEntry } from '../src/api/agent-activity'

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); localStorage.clear(); vi.useRealTimers() })
const session = (id: string, provider: ActivitySummary['provider'] = 'codex'): ActivitySummary => ({ id, provider, source: 'external', title: `Task ${id}`, project: 'Demo', updated_at: Date.now(), state: 'active', preview: 'Working', truncated: false })
const entry = (id: string, text: string, kind: ActivityEntry['kind'] = 'assistant'): ActivityEntry => ({ id, text, kind, tool: kind === 'tool' ? 'shell' : null })
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const list = (sessions: ActivitySummary[]) => ({ schema_version: 1, observed_at: Date.now(), sources: [{ provider: 'codex', status: 'available' }], sessions })
const detail = (row: ActivitySummary, entries = [entry('1', 'Visible reply')]) => ({ schema_version: 1, ...row, entries })
function setup(fetch: typeof globalThis.fetch) {
  const stores = createAppStores({ http: createHttpClient({ fetch }), agentStatusStreamFactory: null })
  stores.bootstrap.status.value = 'ready'
  const Host = defineComponent({
    props: { provider: { type: String as PropType<ExternalActivityProvider>, required: true } },
    setup(props) {
      const activity = useAgentActivity(toRef(props, 'provider'))
      return () => h('div', [h(AgentActivitySidebar, { provider: props.provider, activity, onSelect: activity.select }), h(AgentActivityView, { provider: props.provider, activity })])
    },
  })
  const wrapper = mount(Host, { props: { provider: 'codex' }, global: { plugins: [stores] } }); wrappers.push(wrapper)
  return wrapper
}

describe('live agent content', () => {
  it('refreshes output while preserving the chosen task and expanded tools', async () => {
    vi.useFakeTimers()
    const rows = [session('first'), session('second')]
    let appended = false
    const wrapper = setup(async url => {
      const path = String(url)
      if (path.includes('?provider=')) return response(list(appended ? [...rows].reverse() : rows))
      const row = rows.find(row => path.endsWith(row.id))!
      return response(detail(row, [entry('tool', 'echo safe', 'tool'), entry('answer', appended ? 'Updated reply' : 'Initial reply')]))
    })
    await flushPromises()
    await wrapper.get('[data-activity-session="second"]').trigger('click'); await flushPromises()
    const tool = wrapper.get('details').element as HTMLDetailsElement
    tool.open = true
    appended = true
    await vi.advanceTimersByTimeAsync(2100); await flushPromises()
    expect(wrapper.get('[data-activity-session="second"]').attributes('aria-pressed')).toBe('true')
    expect(wrapper.text()).toContain('Updated reply')
    expect(wrapper.get('details').element).toBe(tool)
    expect(tool.open).toBe(true)
  })

  it('ignores an older provider response after switching', async () => {
    let finishOld!: (value: Response) => void
    let oldSignal: AbortSignal | undefined | null
    const wrapper = setup(async (url, options) => {
      const path = String(url)
      if (path.endsWith('provider=codex')) { oldSignal = options?.signal; return new Promise<Response>(resolve => { finishOld = resolve }) }
      if (path.includes('?provider=')) return response(list([session('claude-task', 'claude')]))
      return response(detail(session('claude-task', 'claude'), [entry('new', 'Claude reply')]))
    })
    await wrapper.setProps({ provider: 'claude' }); await flushPromises()
    finishOld(response(list([session('old-codex')]))); await flushPromises()
    expect(oldSignal?.aborted).toBe(true)
    expect(wrapper.text()).toContain('Claude reply')
    expect(wrapper.find('[data-activity-session="old-codex"]').exists()).toBe(false)
  })

  it('retains visible content when refresh fails and stops polling on unmount', async () => {
    vi.useFakeTimers()
    let failed = false
    const fetch = vi.fn(async (url: Parameters<typeof globalThis.fetch>[0]) => failed ? response({}, 503) : response(String(url).includes('?provider=') ? list([session('one')]) : detail(session('one'))))
    const wrapper = setup(fetch)
    await flushPromises(); failed = true
    await vi.advanceTimersByTimeAsync(2100); await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('last available')
    expect(wrapper.text()).toContain('Visible reply')
    wrapper.unmount(); wrappers.pop()
    const calls = fetch.mock.calls.length
    await vi.advanceTimersByTimeAsync(6000)
    expect(fetch).toHaveBeenCalledTimes(calls)
  })

  it('does not force a reader back to the bottom while inspecting older output', async () => {
    vi.useFakeTimers()
    const wrapper = setup(async url => response(String(url).includes('?provider=') ? list([session('one')]) : detail(session('one'))))
    await flushPromises()
    const transcript = wrapper.get('[data-activity-transcript]')
    const el = transcript.element as HTMLElement
    Object.defineProperties(el, { scrollHeight: { value: 1000 }, clientHeight: { value: 200 } })
    el.scrollTop = 100; await transcript.trigger('scroll')
    await vi.advanceTimersByTimeAsync(2100); await flushPromises()
    expect(el.scrollTop).toBe(100)
    await wrapper.get('.agent-activity__latest').trigger('click')
    expect(el.scrollTop).toBe(1000)
  })

  it('distinguishes missing local tasks from unavailable records', async () => {
    const wrapper = setup(async () => response({ ...list([]), sources: [{ provider: 'codex', status: 'unavailable' }] }))
    await flushPromises()
    expect(wrapper.get('[role="status"]').text()).toContain('could not be read')
    expect(wrapper.get('[data-activity-empty]').text()).toContain('Start a task in Codex')
  })

  it('opens an external provider from a route and returns to the composer on new chat', async () => {
    const router = createAppRouter('memory'); await router.push('/workspace?agent=claude'); await router.isReady()
    const stores = createAppStores({ agentStatusStreamFactory: null })
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores, router] } }); wrappers.push(wrapper)
    expect(wrapper.find('[data-agent-activity-view]').exists()).toBe(true)
    expect(wrapper.find('[data-run-composer]').exists()).toBe(false)
    await wrapper.get('[data-rail-action="new-session"]').trigger('click'); await flushPromises()
    expect(wrapper.find('[data-agent-activity-view]').exists()).toBe(false)
    expect(wrapper.find('[data-run-composer]').exists()).toBe(true)
    expect(router.currentRoute.value.query.agent).toBeUndefined()
  })

  it('persists pins, groups projects and clears unread updates on selection', async () => {
    vi.useFakeTimers()
    const rows = [session('first'), { ...session('second'), updated_at: 1000, state: 'completed' as const }]
    const wrapper = setup(async url => response(String(url).includes('?provider=') ? list(rows) : detail(rows.find(row => String(url).endsWith(row.id))!)))
    await flushPromises()
    await wrapper.get('[data-activity-pin="second"]').trigger('click')
    expect(wrapper.get('[data-activity-section="pinned"]').find('[data-activity-session="second"]').exists()).toBe(true)
    expect(wrapper.get('[data-activity-section="recent"]').find('[data-activity-session="second"]').exists()).toBe(false)
    expect(JSON.parse(localStorage.getItem('orchester:activity:codex')!).pins.second).toBe(true)
    rows[1]!.updated_at = 2000
    await vi.advanceTimersByTimeAsync(2100); await flushPromises()
    expect(wrapper.get('[data-activity-row="second"]').find('.activity-row__unread').exists()).toBe(true)
    await wrapper.get('[data-activity-session="second"]').trigger('click'); await flushPromises()
    expect(wrapper.get('[data-activity-row="second"]').find('.activity-row__unread').exists()).toBe(false)
    await wrapper.get('[data-activity-section="projects"] > button').trigger('click')
    await wrapper.get('[data-activity-project]').trigger('click')
    expect(wrapper.get('[data-activity-section="projects"]').findAll('[data-activity-session]')).toHaveLength(2)
    await wrapper.get('.app-menu__trigger').trigger('click')
    expect(wrapper.findAll('[data-activity-provider]').map(item => item.attributes('data-activity-provider'))).toEqual(['codex', 'claude', 'opencode'])
    expect(wrapper.find('[data-activity-provider="orchester"]').exists()).toBe(false)
  })
})
