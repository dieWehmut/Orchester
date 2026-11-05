import { callId, eventId, runId, type SessionDetailDto, type UiEventEnvelope } from '@orchester/protokoll'
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { createRunStore } from '../src/stores/run'
import { createRunQueue } from '../src/stores/run-queue'
import { createSessionsStore } from '../src/stores/sessions'
import { createAppStores } from '../src/stores/app'
import type { RunsApi } from '../src/api/runs'
import type { HttpClient } from '../src/api/http'
import WorkspaceView from '../src/views/WorkspaceView.vue'

const conversation = 's-11111111111111111111111111111111'
function event(run: string, sequence: number, text: string, user = false): UiEventEnvelope {
  return {
    schema_version: 1, event_id: eventId(`${run}-${sequence}`), run_id: runId(run), sequence,
    occurred_at: '2026-11-05T08:00:00Z',
    kind: user ? { type: 'user_message', text } : { type: 'message', text },
  }
}
const events = [event('first', 1, 'Remember my number: 42', true), event('first', 2, 'I will remember.'), event('second', 3, 'What was it?', true), event('second', 4, '42')]
const detail: SessionDetailDto = {
  schema_version: 1, id: conversation, conversation_id: conversation, source: 'self',
  title: 'Remember my number', recorded_at_unix: 1793865600, agent: 'orchester', model: 'test', outcome: 'success', resumable: true,
  prompt: 'Remember my number: 42', final_text: '42', events, run_ids: ['first', 'second'],
  usage: { input_tokens: 0, output_tokens: 0, cached_input_tokens: 0, reasoning_output_tokens: 0 },
}

describe('durable conversation UI', () => {
  it('restores every saved turn and carries the thread identity on the next request', async () => {
    const start = vi.fn(async () => ({ run_id: 'third', conversation_id: conversation, events_url: '/events/third' }))
    const run = createRunStore({ start } as unknown as RunsApi)
    const queue = createRunQueue(run)
    queue.openConversation(conversation, events)
    expect(queue.view.value.timeline.filter(row => row.type === 'message').map(row => row.text)).toEqual(events.map(row => 'text' in row.kind ? row.kind.text : ''))
    expect(queue.view.value.timeline.some(row => row.type === 'gap')).toBe(false)
    queue.enqueue('Please repeat it')
    await flushPromises()
    expect(start).toHaveBeenCalledWith({ prompt: 'Please repeat it', conversation_id: conversation }, expect.anything())
    run.applyEvent(event('third', 1, 'Please repeat it', true))
    run.applyEvent(event('third', 2, 'Still 42'))
    expect(queue.view.value.timeline.filter(row => row.type === 'message')).toHaveLength(6)
    queue.openConversation(null)
    expect(queue.view.value.timeline).toEqual([])
    expect(run.conversationId.value).toBeNull()
    queue.dispose()
  })

  it('does not mix an in-flight acknowledgement into a newly opened conversation', async () => {
    let resolve!: (value: { run_id: string; conversation_id: string; events_url: string }) => void
    const run = createRunStore({ start: () => new Promise<{ run_id: string; conversation_id: string; events_url: string }>(done => { resolve = done }) } as unknown as RunsApi)
    const queue = createRunQueue(run)
    queue.enqueue('Old request')
    queue.openConversation(conversation, events)
    resolve({ run_id: 'late', conversation_id: 'old', events_url: '/events/late' })
    await flushPromises()
    expect(run.conversationId.value).toBe(conversation)
    expect(queue.items.value).toEqual([])
    expect(queue.view.value.timeline.filter(row => row.type === 'message')).toHaveLength(4)
    queue.dispose()
  })

  it('reconnects a still-running turn after reload without duplicating its saved messages', async () => {
    const live = [event('second', 1, 'What was it?', true), event('second', 2, '42')]
    const run = createRunStore({ snapshot: async () => ({
      run_id: runId('second'), state: 'running', events: live, pending_approvals: [],
      oldest_sequence: 1, latest_sequence: 2, next_sequence: 3, updated_at: '2026-11-05T08:00:00Z',
    }) } as unknown as RunsApi)
    const queue = createRunQueue(run)
    queue.openConversation(conversation, events)
    await run.reconnect({ run_id: 'second', conversation_id: conversation, events_url: '/events/second' })
    expect(queue.view.value.timeline.filter(row => row.type === 'message')).toHaveLength(4)
    expect(run.lifecycle.value).toBe('running')
    run.applyEvent({ ...event('second', 3, ''), kind: { type: 'run_stopped', reason: 'succeeded' } })
    expect(run.lifecycle.value).toBe('completed')
    queue.dispose()
  })

  it('keeps repeated public tool aliases distinct across conversation turns', () => {
    const run = createRunStore()
    const queue = createRunQueue(run)
    const tool = (id: string, sequence: number): UiEventEnvelope => ({
      ...event(id, sequence, ''), kind: { type: 'tool_call', call_id: callId('call-1'), name: 'read_file', state: 'succeeded' },
    })
    queue.openConversation(conversation, [tool('first', 1), tool('second', 2)])
    const keys = queue.view.value.timeline.map(item => item.key)
    expect(keys).toHaveLength(2)
    expect(new Set(keys).size).toBe(2)
    queue.dispose()
  })

  it('recovers the latest server conversation with no browser storage, including a fresh application instance', async () => {
    localStorage.clear()
    const api = { list: vi.fn(async () => ({ schema_version: 1 as const, items: [detail], next_cursor: null })), detail: vi.fn(async () => detail) }
    const first = createSessionsStore(api)
    await first.load()
    await first.restoreSelection('test-workspace')
    expect(first.selected.value?.events).toHaveLength(4)
    const restarted = createSessionsStore(api)
    await restarted.load()
    await restarted.restoreSelection('test-workspace')
    expect(restarted.selectedId.value).toBe(conversation)
    await restarted.select(null)
    const newChat = createSessionsStore(api)
    await newChat.load()
    await newChat.restoreSelection('test-workspace')
    expect(newChat.selected.value).toBeNull()
    localStorage.clear()
  })

  it('opens saved self-agent messages in the live composer so history can be continued', async () => {
    const stores = createAppStores({ http: { get: async () => detail } as unknown as HttpClient, agentStatusStreamFactory: null })
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    await stores.sessions.select(conversation)
    await flushPromises()
    expect(wrapper.find('[data-run-composer]').exists()).toBe(true)
    expect(wrapper.text()).toContain('Remember my number: 42')
    expect(wrapper.text()).toContain('What was it?')
    expect(stores.run.conversationId.value).toBe(conversation)
    wrapper.unmount()
    stores.stop()
  })

  it('keeps the active conversation visible if the sidebar history refresh fails', async () => {
    const stores = createAppStores({ http: { get: async () => { throw new Error('temporary history failure') } } as unknown as HttpClient, agentStatusStreamFactory: null })
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    stores.run.conversationId.value = conversation
    stores.run.applyEvent(event('live', 1, 'The live reply remains visible'))
    await flushPromises()
    expect(wrapper.find('[data-run-composer]').exists()).toBe(true)
    expect(wrapper.text()).toContain('The live reply remains visible')
    wrapper.unmount()
    stores.stop()
  })
})
