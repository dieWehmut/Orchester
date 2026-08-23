import {
  fixtureEnvelope,
  runId,
  type RunSnapshotDto,
} from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createRunStore, type RunSocketFactory } from '../src/stores/run'
import type { RunsApi } from '../src/api/runs'

function createFakeRunSocketFactory() {
  const sockets: Array<{
    options: Parameters<RunSocketFactory>[0]
    connect: ReturnType<typeof vi.fn>
    close: ReturnType<typeof vi.fn>
  }> = []
  const factory: RunSocketFactory = (options) => {
    const socket = {
      options,
      connect: vi.fn(async () => {
        options.onStatus?.('connected')
      }),
      close: vi.fn(),
      status: 'idle' as const,
    }
    sockets.push(socket)
    return socket
  }
  return { factory, sockets }
}

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

  it('hydrates the snapshot and opens the server-issued event stream after submit', async () => {
    const run = runId('run-live')
    const snapshot: RunSnapshotDto = {
      run_id: run,
      state: 'running',
      events: [{ ...fixtureEnvelope(1, { type: 'run_started', title: 'Live run' }), run_id: run }],
      pending_approvals: [],
      oldest_sequence: 1,
      latest_sequence: 1,
      next_sequence: 2,
      updated_at: '2026-08-19T00:00:01.000Z',
    }
    const start = vi.fn(async () => ({ run_id: run, events_url: 'wss://runtime/events/live' }))
    const snapshotRequest = vi.fn(async () => snapshot)
    const api = { start, snapshot: snapshotRequest } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await expect(store.submit('Inspect the live workspace')).resolves.toMatchObject({ run_id: run })
    await vi.waitFor(() => expect(snapshotRequest).toHaveBeenCalledWith(run))

    expect(store.view.value.title).toBe('Live run')
    expect(sockets.sockets).toHaveLength(1)
    expect(sockets.sockets[0]?.options.ticketProvider()).toBe('wss://runtime/events/live')
    expect(sockets.sockets[0]?.options.afterSequence?.()).toBe(1)
    expect(store.connectionStatus.value).toBe('connected')

    sockets.sockets[0]?.options.onEvent?.({
      ...fixtureEnvelope(2, { type: 'message', text: 'The workspace is ready.' }),
      run_id: run,
    })
    expect(store.events.value.at(-1)?.kind).toMatchObject({
      type: 'message',
      text: 'The workspace is ready.',
    })
  })

  it('closes the active event stream when the store stops', async () => {
    const run = runId('run-stop')
    const start = vi.fn(async () => ({ run_id: run, events_url: 'wss://runtime/events/stop' }))
    const api = {
      start,
      snapshot: vi.fn(async () => ({
        run_id: run,
        state: 'running' as const,
        events: [],
        pending_approvals: [],
        oldest_sequence: 0,
        latest_sequence: 0,
        next_sequence: 1,
        updated_at: '2026-08-19T00:00:00.000Z',
      })),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await store.submit('Stop after start')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(1))
    store.stop()
    expect(sockets.sockets[0]?.close).toHaveBeenCalledOnce()
  })

  it('does not attach a stale stream when the store stops before snapshot hydration', async () => {
    const run = runId('run-stale')
    let resolveSnapshot!: (snapshot: RunSnapshotDto) => void
    const start = vi.fn(async () => ({ run_id: run, events_url: 'wss://runtime/events/stale' }))
    const snapshot = new Promise<RunSnapshotDto>((resolve) => {
      resolveSnapshot = resolve
    })
    const api = { start, snapshot: vi.fn(() => snapshot) } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await store.submit('Stop before hydration')
    store.stop()
    resolveSnapshot({
      run_id: run,
      state: 'running',
      events: [],
      pending_approvals: [],
      oldest_sequence: 0,
      latest_sequence: 0,
      next_sequence: 1,
      updated_at: '2026-08-19T00:00:00.000Z',
    })
    await Promise.resolve()
    await Promise.resolve()
    expect(sockets.sockets).toHaveLength(0)
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
