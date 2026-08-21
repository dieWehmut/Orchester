import {
  fixtureEnvelope,
  runId,
  type RunSnapshotDto,
} from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createRunStore } from '../src/stores/run'
import type { RunsApi } from '../src/api/runs'

describe('run store', () => {
  it('submits once, stores the returned run id, and rejects a duplicate while busy', async () => {
    const start = vi.fn(async () => ({ run_id: 'run-submit', events_url: '/events/run-submit' }))
    const api = { start } as unknown as RunsApi
    const store = createRunStore(api, { idempotencyKey: () => 'request-1' })

    const first = store.submit(' Inspect the workspace ')
    const duplicate = store.submit('Inspect again')

    expect(await duplicate).toBeNull()
    expect(await first).toMatchObject({ run_id: 'run-submit' })
    expect(start).toHaveBeenCalledTimes(1)
    expect(start).toHaveBeenCalledWith(
      { prompt: 'Inspect the workspace' },
      { idempotencyKey: 'request-1' },
    )
    expect(store.runId.value).toBe('run-submit')
    expect(store.lifecycle.value).toBe('running')
    expect(store.conversationStarted.value).toBe(true)
  })

  it('cancels the active run and reaches a terminal lifecycle state', async () => {
    const cancel = vi.fn(async () => ({ run_id: 'run-cancel', stopped: true, usage: {} }))
    const api = { cancel } as unknown as RunsApi
    const store = createRunStore(api)
    store.runId.value = 'run-cancel'
    store.lifecycle.value = 'running'

    await expect(store.cancel()).resolves.toMatchObject({ stopped: true })
    expect(cancel).toHaveBeenCalledWith('run-cancel')
    expect(store.lifecycle.value).toBe('completed')
    expect(store.connectionStatus.value).toBe('closed')
  })

  it('keeps the first event for a replayed sequence and exposes a gap', () => {
    const store = createRunStore()
    const first = fixtureEnvelope(1, { type: 'run_started', title: 'First' })
    const duplicate = {
      ...first,
      kind: { type: 'run_started', title: 'Replay' } as const,
    }

    expect(store.applyEvent(first)).toBe(true)
    expect(store.applyEvent(duplicate)).toBe(false)
    expect(store.applyEvent(fixtureEnvelope(3, { type: 'message', text: 'buffered' }))).toBe(true)

    expect(store.view.value.title).toBe('First')
    expect(store.view.value.latestSequence).toBe(1)
    expect(store.projectionStatus.value).toBe('gap')
    expect(store.events.value).toHaveLength(2)
    expect(store.conversationStarted.value).toBe(true)
  })

  it('replaces the journal on a bounded snapshot', () => {
    const run = runId('run-store')
    const snapshot: RunSnapshotDto = {
      run_id: run,
      state: 'succeeded',
      events: [
        { ...fixtureEnvelope(4, { type: 'run_started', title: 'Fresh' }), run_id: run },
        { ...fixtureEnvelope(5, { type: 'run_stopped', reason: 'succeeded' }), run_id: run },
      ],
      pending_approvals: [],
      oldest_sequence: 4,
      latest_sequence: 5,
      next_sequence: 6,
      updated_at: '2026-08-19T00:00:05.000Z',
    }

    const store = createRunStore()
    store.applyEvent(fixtureEnvelope(1, { type: 'run_started', title: 'Old' }))
    store.applySnapshot(snapshot)

    expect(store.view.value.runId).toBe(run)
    expect(store.view.value.title).toBe('Fresh')
    expect(store.view.value.status).toBe('succeeded')
    expect(store.events.value.map((event) => event.sequence)).toEqual([4, 5])
    expect(store.projectionStatus.value).toBe('ready')
  })

  it('rejects cross-run events and preserves the current projection', () => {
    const store = createRunStore()
    store.applyEvent(fixtureEnvelope(1, { type: 'run_started' }))

    expect(() => store.applyEvent({ ...fixtureEnvelope(2, { type: 'message', text: 'other' }), run_id: runId('other') })).toThrow(
      RangeError,
    )
    expect(store.events.value).toHaveLength(1)
  })

  it('tracks connection and recoverable projection errors independently', () => {
    const store = createRunStore()
    store.setConnectionStatus('reconnecting')
    store.setError(new Error('temporary'))

    expect(store.connectionStatus.value).toBe('reconnecting')
    expect(store.projectionStatus.value).toBe('error')
    expect(store.error.value?.message).toBe('temporary')

    store.clearError()
    expect(store.projectionStatus.value).toBe('idle')
    expect(store.connectionStatus.value).toBe('reconnecting')
  })

  it('clears conversation state only when starting a new session', async () => {
    const store = createRunStore()
    store.applyEvent(fixtureEnvelope(1, { type: 'run_started' }))

    expect(store.conversationStarted.value).toBe(true)
    store.reset()
    expect(store.conversationStarted.value).toBe(false)
  })
})
