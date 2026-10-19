import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { createRunStore } from '../src/stores/run'
import { createRunQueue } from '../src/stores/run-queue'
import type { RunsApi } from '../src/api/runs'
import UserMessage from '../src/components/run/UserMessage.vue'
import QueuedPrompts from '../src/components/run/QueuedPrompts.vue'

function setup(start = vi.fn(async () => ({ run_id: 'run-1', events_url: '/events' }))) {
  const run = createRunStore({ start } as unknown as RunsApi)
  return { run, queue: createRunQueue(run), start }
}

describe('queued messages', () => {
  it('accepts rapid drafts once, starts them in order only after completion', async () => {
    const { run, queue, start } = setup()
    queue.enqueue('First'); queue.enqueue('Second'); queue.enqueue('Third')
    await flushPromises()
    expect(start.mock.calls).toHaveLength(1)
    expect(queue.items.value.map(item => item.prompt)).toEqual(['Second', 'Third'])
    run.lifecycle.value = 'completed'
    await flushPromises()
    expect(start.mock.calls).toHaveLength(2)
    expect(queue.items.value.map(item => item.prompt)).toEqual(['Third'])
    run.lifecycle.value = 'completed'
    await flushPromises()
    expect(start.mock.calls.map(call => (call as unknown as [{ prompt: string }])[0].prompt)).toEqual(['First', 'Second', 'Third'])
    expect(queue.items.value).toEqual([])
    queue.dispose()
  })

  it('retains an unacknowledged submission and retries the same request identity', async () => {
    const start = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue({ run_id: 'retried', events_url: '/events' })
    const { queue } = setup(start)
    queue.enqueue('Keep this'); queue.enqueue('Then this')
    await flushPromises()
    expect(queue.items.value[0]).toMatchObject({ prompt: 'Keep this', status: 'failed' })
    expect(queue.paused.value).toBe(true)
    queue.resume(); await flushPromises()
    expect(start.mock.calls[0]![1]).toEqual(start.mock.calls[1]![1])
    expect(queue.items.value.map(item => item.prompt)).toEqual(['Then this'])
    queue.dispose()
  })

  it('drains even when a run has already completed before its start response is applied', async () => {
    const start = vi.fn(async () => ({ run_id: `run-${Math.random()}`, events_url: '/events' }))
    const run = createRunStore({ start, snapshot: async (id: string) => ({ run_id: id, state: 'succeeded', events: [], pending_approvals: [], oldest_sequence: 0, latest_sequence: 0, next_sequence: 1, updated_at: '2026-10-18T00:00:00Z' }) } as unknown as RunsApi)
    const queue = createRunQueue(run)
    queue.enqueue('One'); queue.enqueue('Two'); queue.enqueue('Three')
    await flushPromises()
    expect(start).toHaveBeenCalledTimes(3)
    expect(queue.items.value).toEqual([])
    queue.dispose()
  })

  it.each(['failed', 'cancelled', 'paused', 'cancelling'] as const)('pauses on %s and permits removing or editing waiting drafts', async state => {
    const { run, queue, start } = setup()
    queue.enqueue('First'); queue.enqueue('Edit this'); queue.enqueue('Remove this')
    await flushPromises()
    run.lifecycle.value = state
    await flushPromises()
    expect(queue.paused.value).toBe(true)
    const [edit, remove] = queue.items.value
    expect(queue.remove(edit!.id)).toBe('Edit this')
    expect(queue.remove(remove!.id)).toBe('Remove this')
    expect(start).toHaveBeenCalledTimes(1)
    queue.dispose()
  })

  it('cannot remove a request being acknowledged and does not drain after disposal', async () => {
    let resolve!: (value: { run_id: string; events_url: string }) => void
    const start = vi.fn(() => new Promise<{ run_id: string; events_url: string }>(done => { resolve = done }))
    const { run, queue } = setup(start)
    queue.enqueue('First'); queue.enqueue('Next')
    expect(queue.remove(queue.items.value[0]!.id)).toBeNull()
    queue.dispose(); resolve({ run_id: 'first', events_url: '/events' })
    await flushPromises(); run.lifecycle.value = 'completed'; await flushPromises()
    expect(start).toHaveBeenCalledTimes(1)
  })

  it('offers queue editing, removal and explicit resume', async () => {
    const wrapper = mount(QueuedPrompts, { props: { items: [{ id: 'one', prompt: 'Read the tree', status: 'waiting' }], paused: true } })
    await wrapper.get('[aria-label="Edit queued message"]').trigger('click')
    await wrapper.get('[aria-label="Remove queued message"]').trigger('click')
    await wrapper.get('[data-queue-resume]').trigger('click')
    expect(wrapper.emitted('edit')).toEqual([['one']]); expect(wrapper.emitted('remove')).toEqual([['one']]); expect(wrapper.emitted('resume')).toHaveLength(1)
  })

  it('shows the real message time below the bubble and edits without sending', async () => {
    const wrapper = mount(UserMessage, { props: { text: '继续', timestamp: '2026-10-18T08:37:00Z' } })
    expect(wrapper.get('[data-message-time]').attributes('datetime')).toBe('2026-10-18T08:37:00.000Z')
    expect(wrapper.get('[data-message-plain]').element.contains(wrapper.get('[data-message-actions]').element)).toBe(false)
    await wrapper.get('[data-message-action="edit"]').trigger('click')
    expect(wrapper.emitted('edit')).toEqual([['继续']])
    expect(wrapper.emitted('submit')).toBeUndefined()
  })
})
