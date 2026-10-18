import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import AgentConversation from '../src/features/agent-presence/components/AgentConversation.vue'
import { conversationItems, visibleAnswer, workSteps } from '../src/features/agent-presence/conversation'
import { createAgentActivityApi, type ActivityDetail, type ActivityEntry } from '../src/api/agent-activity'
import { createHttpClient } from '../src/api/http'

const entry = (id: string, kind: ActivityEntry['kind'], text: string, extra: Partial<ActivityEntry> = {}): ActivityEntry => ({ id, kind, text, tool: null, ...extra })
const entries = [
  entry('prompt', 'user', '继续', { turn_id: 'turn' }),
  entry('progress', 'assistant', '正在检查 **安装版本**。', { phase: 'commentary', turn_id: 'turn' }),
  entry('tool', 'tool', 'echo verified', { tool: 'shell', turn_id: 'turn' }),
  entry('output', 'result', 'verified', { turn_id: 'turn' }),
  entry('answer', 'assistant', '已完成，并重装本机 **0.1.8**：\n\n- 移除品牌区。\n- 已推送远程 `main`。\n\n[查看文档](https://example.com/docs)\n\n```ts\nconst ready = true\n```', { phase: 'final', turn_id: 'turn' }),
]
const detail = (extra: Partial<ActivityDetail> = {}): ActivityDetail => ({ schema_version: 1, id: 'session', provider: 'codex', source: 'external', title: '同步仓库', project: 'Orchester', updated_at: 2000, state: 'completed', preview: '', truncated: false, entries, turns: [{ id: 'turn', started_at: 1000, completed_at: 1320000 }], ...extra })

describe('source conversation rendering', () => {
  it('renders the reference as a prompt, expanded latest work, Markdown answer and working copy action', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    const wrapper = mount(AgentConversation, { props: { detail: detail() } })
    try {
      expect(wrapper.get('[data-activity-entry="user"]').text()).toBe('继续')
      expect((wrapper.get('[data-work-disclosure]').element as HTMLDetailsElement).open).toBe(true)
      expect(wrapper.get('[data-work-disclosure] > summary').text()).toBe('Worked for 21m 59s')
      const answer = wrapper.get('.agent-conversation__answer')
      expect(answer.findAll('li')).toHaveLength(2)
      expect(answer.get('strong').text()).toBe('0.1.8')
      expect(answer.get('[data-markdown-inline-code]').text()).toBe('main')
      expect(answer.get('pre code').text()).toBe('const ready = true')
      expect(answer.get('a').attributes('rel')).toBe('noopener noreferrer')
      await answer.get('[data-message-copy]').trigger('click')
      expect(writeText).toHaveBeenCalledWith(entries.at(-1)?.text)
      expect(wrapper.find('.agent-activity__speaker').exists()).toBe(false)
    } finally { wrapper.unmount(); vi.unstubAllGlobals() }
  })

  it('preserves nested disclosure elements while a source turn finishes', async () => {
    const wrapper = mount(AgentConversation, { props: { detail: detail({ state: 'active', entries: entries.slice(0, -1), turns: [{ id: 'turn', started_at: 1000 }] }) } })
    const disclosure = wrapper.get('[data-work-disclosure]').element as HTMLDetailsElement
    const tool = wrapper.get('[data-tool-disclosure]').element as HTMLDetailsElement
    disclosure.open = true; tool.open = true
    expect(disclosure.open).toBe(true)
    expect(wrapper.get('[data-work-disclosure]').text()).toContain('正在检查')
    await wrapper.setProps({ detail: detail() })
    expect(wrapper.get('[data-work-disclosure]').element).toBe(disclosure)
    expect(wrapper.get('[data-tool-disclosure]').element).toBe(tool)
    expect(disclosure.open).toBe(true)
    expect(tool.open).toBe(true)
    expect(wrapper.find('[data-work-preview]').exists()).toBe(false)
    expect(wrapper.get('.agent-conversation__answer').text()).toContain('已完成')
    wrapper.unmount()
  })

  it.each(['codex', 'claude', 'opencode'] as const)('renders Markdown for %s and treats HTML as text', provider => {
    const wrapper = mount(AgentConversation, { props: { detail: detail({ provider, entries: [entry('reply', 'assistant', '# 标题\n\n<script>alert(1)</script>\n\n| 项目 | 状态 |\n| --- | --- |\n| 安装 | 完成 |')] }) } })
    expect(wrapper.get('[data-conversation-provider]').attributes('data-conversation-provider')).toBe(provider)
    expect(wrapper.get('h3').text()).toBe('标题')
    expect(wrapper.get('table').text()).toContain('完成')
    expect(wrapper.find('script').exists()).toBe(false)
    wrapper.unmount()
  })

  it('keeps legacy, truncated and consecutive prompt content in order without inventing elapsed time', () => {
    const legacy = [entry('a', 'assistant', 'Inspecting'), entry('b', 'tool', 'read file'), entry('c', 'assistant', 'Answer'), entry('d', 'user', 'next'), entry('e', 'user', 'clarification'), entry('f', 'assistant', 'Reply')]
    const items = conversationItems(legacy)
    expect(items.map(item => item.kind)).toEqual(['work', 'assistant', 'user', 'user', 'assistant'])
    expect(items[0]).not.toHaveProperty('duration', expect.any(Number))
    const mixed = conversationItems([entry('a', 'assistant', 'Final', { phase: 'final' }), entry('b', 'tool', 'later tool')])
    expect(mixed.map(item => item.kind)).toEqual(['assistant', 'work'])
    expect(conversationItems(entries, [{ id: 'turn', started_at: 10, completed_at: 1 }])[1]).not.toHaveProperty('duration', expect.any(Number))
  })

  it('hides only trailing citation bookkeeping and keeps literal code examples', () => {
    const citation = '<oai-mem-citation>\nmetadata\n</oai-mem-citation>'
    expect(visibleAnswer(`Answer\n\n${citation}`)).toBe('Answer')
    expect(visibleAnswer(`\`\`\`xml\n${citation}`)).toBe(`\`\`\`xml\n${citation}`)
    expect(visibleAnswer(`${citation}\nOther prose`)).toBe(`${citation}\nOther prose`)
  })

  it('pairs interleaved tool results and leaves orphan results readable', () => {
    const steps = workSteps([entry('a','tool','first',{ call_id:'a' }),entry('b','tool','second',{ call_id:'b' }),entry('br','result','second result',{ call_id:'b' }),entry('ar','result','first result',{ call_id:'a' }),entry('orphan','result','unpaired',{ call_id:'missing' })])
    expect(steps.map(step => [step.id, step.results.map(result => result.id)])).toEqual([['a',['ar']],['b',['br']],['orphan',[]]])
  })

  it('renders known user timestamps and copies only visible final text', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    const wrapper = mount(AgentConversation, { props: { detail: detail({ entries: [entry('u','user','Continue',{timestamp: 1792112400000}), entry('a','assistant','Done\n\n<oai-mem-citation>\nmetadata\n</oai-mem-citation>',{ phase:'final' })] }) } })
    expect(wrapper.find('[data-conversation-date]').exists()).toBe(true)
    expect(wrapper.get('[data-message-time]').text()).toMatch(/\d{2}:\d{2}/)
    await wrapper.get('.agent-conversation__answer [data-message-copy]').trigger('click')
    expect(writeText).toHaveBeenCalledWith('Done')
    wrapper.unmount(); vi.unstubAllGlobals()
  })

  it('accepts old metadata-free servers and rejects malformed optional timestamps', async () => {
    let body = detail({ entries: [entry('old', 'assistant', 'Legacy')] })
    delete body.turns
    const api = createAgentActivityApi(createHttpClient({ fetch: async () => new Response(JSON.stringify(body), { headers: { 'content-type': 'application/json' } }) }))
    await expect(api.detail('session', new AbortController().signal)).resolves.toMatchObject({ id: 'session' })
    body = detail({ entries: [entry('bad', 'assistant', 'text', { timestamp: 1e20 })] })
    await expect(api.detail('session', new AbortController().signal)).rejects.toThrow('Invalid activity detail')
  })

  it.each(['codex', 'claude', 'opencode'] as const)('updates prose, pending tools and elapsed time in place for %s', async provider => {
    vi.useFakeTimers(); vi.setSystemTime(5000)
    const initial = detail({ provider, state: 'active', entries: entries.slice(0, 3), turns: [{ id: 'turn', started_at: 1000 }] })
    const wrapper = mount(AgentConversation, { props: { detail: initial } })
    try {
      const work = wrapper.get('[data-work-disclosure]').element as HTMLDetailsElement
      const tool = wrapper.get('[data-tool-disclosure]').element as HTMLDetailsElement
      expect(work.open).toBe(true)
      expect(wrapper.get('[data-work-disclosure] > summary').text()).toBe('Working · 4s')
      expect(wrapper.find('[data-tool-pending="true"]').exists()).toBe(true)
      await vi.advanceTimersByTimeAsync(2000)
      expect(wrapper.get('[data-work-disclosure] > summary').text()).toBe('Working · 6s')
      tool.open = true
      await wrapper.setProps({ detail: { ...initial, entries: [...entries.slice(0, 4), entry('more', 'assistant', '下一步 **安装**', { phase: 'commentary', turn_id: 'turn' })] } })
      expect(wrapper.get('[data-tool-disclosure]').element).toBe(tool)
      expect(tool.open).toBe(true)
      expect(wrapper.find('[data-tool-pending="true"]').exists()).toBe(false)
      expect(wrapper.get('[data-tool-result]').text()).toBe('verified')
      expect(wrapper.get('[data-work-disclosure]').text()).toContain('下一步 安装')
      work.open = false; await wrapper.get('[data-work-disclosure]').trigger('toggle')
      await wrapper.setProps({ detail: detail({ provider }) })
      expect(wrapper.get('[data-work-disclosure]').element).toBe(work)
      expect(work.open).toBe(false)
      expect(wrapper.find('[data-conversation-running]').exists()).toBe(false)
      await vi.advanceTimersByTimeAsync(2000)
      expect(wrapper.get('[data-work-disclosure] > summary').text()).toBe('Worked for 21m 59s')
    } finally { wrapper.unmount(); vi.useRealTimers() }
  })

  it('separates distinct source turns and expands only the latest round on arrival', () => {
    const next = entries.map(item => ({ ...item, id: `next-${item.id}`, turn_id: 'next-turn' }))
    const wrapper = mount(AgentConversation, { props: { detail: detail({ entries: [...entries, ...next] }) } })
    expect(wrapper.findAll('[data-work-disclosure]').map(item => (item.element as HTMLDetailsElement).open)).toEqual([false, true])
    wrapper.unmount()
    const work = conversationItems([entry('a', 'tool', 'first', { turn_id: 'first' }), entry('b', 'tool', 'second', { turn_id: 'second' })])
    expect(work).toHaveLength(2)
  })

  it('identifies integrations, shows waiting activity and pauses it when updates are unavailable', async () => {
    const wrapper = mount(AgentConversation, { props: { detail: detail({ state: 'active', entries: [entry('u', 'user', '继续')] }) } })
    expect(wrapper.find('[data-conversation-running]').exists()).toBe(true)
    await wrapper.setProps({ detail: detail({ state: 'active', entries: [entry('t', 'tool', '{}', { tool: 'mcp__playwright__browser_navigate' }), entry('tail', 'assistant', 'Still checking')] }) })
    expect(wrapper.get('[data-integration="Playwright"] summary').text()).toContain('Playwright')
    expect(wrapper.find('[data-tool-pending="true"]').exists()).toBe(true)
    await wrapper.setProps({ stale: true })
    expect(wrapper.find('[data-conversation-running]').exists()).toBe(false)
    expect(wrapper.find('[data-tool-pending="true"]').exists()).toBe(false)
    wrapper.unmount()
  })
})
