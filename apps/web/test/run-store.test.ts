import {
  fixtureEnvelope,
  runId,
  type RunSnapshotDto,
  type RunSummaryDto,
  type StartRunResponse,
} from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createRunStore, type RunSocketFactory } from '../src/stores/run'
import type { RunsApi } from '../src/api/runs'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function snapshotFor(id: string, state: RunSnapshotDto['state'] = 'running'): RunSnapshotDto {
  return {
    run_id: runId(id),
    state,
    events: [],
    pending_approvals: [],
    oldest_sequence: 0,
    latest_sequence: 0,
    next_sequence: 1,
    updated_at: '2026-08-19T00:00:00.000Z',
  }
}

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

  it('reuses the idempotency key when a failed submission is retried', async () => {
    const start = vi
      .fn()
      .mockRejectedValueOnce(new Error('network lost'))
      .mockResolvedValueOnce({ run_id: 'run-retry', events_url: '/events/run-retry' })
    const makeKey = vi.fn(() => 'request-retry')
    const api = { start } as unknown as RunsApi
    const store = createRunStore(api, { idempotencyKey: makeKey })

    await expect(store.submit('Retry the same submission')).resolves.toBeNull()
    await expect(store.submit('  Retry the same submission  ')).resolves.toMatchObject({
      run_id: 'run-retry',
    })

    expect(makeKey).toHaveBeenCalledOnce()
    expect(start).toHaveBeenNthCalledWith(
      1,
      { prompt: 'Retry the same submission' },
      { idempotencyKey: 'request-retry' },
    )
    expect(start).toHaveBeenNthCalledWith(
      2,
      { prompt: 'Retry the same submission' },
      { idempotencyKey: 'request-retry' },
    )
  })

  it('creates a fresh submission intent when the prompt changes after a lost response', async () => {
    const start = vi.fn().mockRejectedValue(new Error('network lost'))
    const makeKey = vi.fn()
      .mockReturnValueOnce('first-intent')
      .mockReturnValueOnce('edited-intent')
      .mockReturnValueOnce('new-first-intent')
    const store = createRunStore({ start } as unknown as RunsApi, { idempotencyKey: makeKey })

    await store.submit('Original prompt')
    await store.submit('Edited prompt')
    await store.submit('Original prompt')

    expect(start.mock.calls.map((call) => call[1]?.idempotencyKey)).toEqual([
      'first-intent', 'edited-intent', 'new-first-intent',
    ])
  })

  it('forgets an unacknowledged submission intent when the session resets', async () => {
    const start = vi.fn().mockRejectedValue(new Error('network lost'))
    const makeKey = vi.fn().mockReturnValueOnce('old-session').mockReturnValueOnce('new-session')
    const store = createRunStore({ start } as unknown as RunsApi, { idempotencyKey: makeKey })

    await store.submit('The same prompt')
    store.reset()
    await store.submit('The same prompt')

    expect(start.mock.calls.map((call) => call[1]?.idempotencyKey)).toEqual(['old-session', 'new-session'])
  })

  it('uses a new key for an intentional run after the same prompt was acknowledged', async () => {
    const start = vi.fn()
      .mockResolvedValueOnce({ run_id: 'run-first-intent', events_url: '/events/first-intent' })
      .mockResolvedValueOnce({ run_id: 'run-next-intent', events_url: '/events/next-intent' })
    const makeKey = vi.fn().mockReturnValueOnce('first-intent').mockReturnValueOnce('next-intent')
    const api = {
      start,
      snapshot: vi.fn(async (id: string) => snapshotFor(id, 'succeeded')),
    } as unknown as RunsApi
    const store = createRunStore(api, { idempotencyKey: makeKey })

    await store.submit('Repeat deliberately')
    await vi.waitFor(() => expect(store.lifecycle.value).toBe('completed'))
    await store.submit('Repeat deliberately')

    expect(start.mock.calls.map((call) => call[1]?.idempotencyKey)).toEqual(['first-intent', 'next-intent'])
  })

  it.each(['resolve', 'reject'] as const)('ignores a late start %s after reset and a new submission', async (settlement) => {
    const oldRequest = deferred<StartRunResponse>()
    const start = vi.fn()
      .mockReturnValueOnce(oldRequest.promise)
      .mockResolvedValueOnce({ run_id: 'run-current', events_url: '/events/current' })
    const store = createRunStore({ start } as unknown as RunsApi)

    const oldSubmission = store.submit('Old session')
    store.reset()
    await store.submit('Current session')
    if (settlement === 'resolve') oldRequest.resolve({ run_id: 'run-old', events_url: '/events/old' })
    else oldRequest.reject(new Error('Old session failed'))

    await expect(oldSubmission).resolves.toBeNull()
    expect(store.runId.value).toBe('run-current')
    expect(store.lifecycle.value).toBe('running')
    expect(store.connectionStatus.value).toBe('connecting')
    expect(store.error.value).toBeNull()
  })

  it.each(['resolve', 'reject'] as const)('ignores a late start %s after stop', async (settlement) => {
    const request = deferred<StartRunResponse>()
    const snapshot = vi.fn()
    const api = { start: vi.fn(() => request.promise), snapshot } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    const submission = store.submit('Stop while submitting')
    store.stop()
    const stoppedLifecycle = store.lifecycle.value
    const stoppedConnection = store.connectionStatus.value
    if (settlement === 'resolve') request.resolve({ run_id: 'run-late', events_url: '/events/late' })
    else request.reject(new Error('Late rejection'))

    await expect(submission).resolves.toBeNull()
    expect(store.runId.value).toBeNull()
    expect(store.lifecycle.value).toBe(stoppedLifecycle)
    expect(store.connectionStatus.value).toBe(stoppedConnection)
    expect(store.error.value).toBeNull()
    expect(snapshot).not.toHaveBeenCalled()
    expect(sockets.sockets).toHaveLength(0)
  })

  it.each([
    ['succeeded', 'completed'],
    ['failed', 'failed'],
    ['cancelled', 'cancelled'],
    ['paused', 'paused'],
  ] as const)('restores a reused %s run without opening a new event stream', async (state, expectedLifecycle) => {
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-reused', events_url: '/events/reused' })),
      snapshot: vi.fn(async () => snapshotFor('run-reused', state)),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await store.submit('Recover a completed request')
    await vi.waitFor(() => expect(store.lifecycle.value).toBe(expectedLifecycle))

    expect(store.view.value.status).toBe(state === 'paused' ? 'interrupted_unknown_outcome' : state)
    expect(store.connectionStatus.value).toBe('closed')
    expect(sockets.sockets).toHaveLength(0)
  })

  it('clears the previous run projection before a new run receives events', async () => {
    const start = vi.fn(async () => ({ run_id: 'run-next', events_url: '/events/next' }))
    const store = createRunStore({ start } as unknown as RunsApi)
    store.applySnapshot({
      ...snapshotFor('run-previous', 'succeeded'),
      events: [{ ...fixtureEnvelope(1, { type: 'message', text: 'Previous run output' }), run_id: runId('run-previous') }],
      oldest_sequence: 1,
      latest_sequence: 1,
      next_sequence: 2,
    })

    await store.submit('New run')
    expect(store.events.value).toHaveLength(0)
    expect(store.view.value.timeline).toHaveLength(0)
    expect(store.applyEvent({
      ...fixtureEnvelope(1, { type: 'run_started', title: 'New run' }),
      run_id: runId('run-next'),
    })).toBe(true)
    expect(store.view.value.title).toBe('New run')
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

  it('keeps the run active when only the event transport fails', async () => {
    const run = runId('run-transport-failure')
    const api = {
      start: vi.fn(async () => ({
        run_id: run,
        events_url: 'wss://runtime/events/transport-failure',
      })),
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
    const transportError = new Error('event transport unavailable')
    const runSocketFactory: RunSocketFactory = (options) => ({
      status: 'idle',
      connect: vi.fn(async () => {
        options.onStatus?.('fatal')
        options.onError?.(transportError)
        throw transportError
      }),
      close: vi.fn(),
    })
    const store = createRunStore(api, { runSocketFactory })

    await store.submit('Keep the run active')
    await vi.waitFor(() => expect(store.error.value?.message).toBe(transportError.message))

    expect(store.lifecycle.value).toBe('running')
    expect(store.connectionStatus.value).toBe('error')
  })

  it.each([
    ['succeeded', 'completed'],
    ['failed', 'failed'],
    ['cancelled', 'cancelled'],
  ] as const)('settles a %s stream and ignores its late callbacks', async (reason, expectedLifecycle) => {
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-terminal', events_url: '/events/terminal' })),
      snapshot: vi.fn(async () => snapshotFor('run-terminal')),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })
    await store.submit('Finish the run')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(1))
    const socket = sockets.sockets[0]!
    socket.options.onEvent?.({
      ...fixtureEnvelope(1, { type: 'run_stopped', reason }),
      run_id: runId('run-terminal'),
    })
    socket.options.onStatus?.('reconnecting')
    socket.options.onError?.(new Error('Late transport error'))
    socket.options.onEvent?.({
      ...fixtureEnvelope(2, { type: 'message', text: 'Late output' }),
      run_id: runId('run-terminal'),
    })

    expect(store.lifecycle.value).toBe(expectedLifecycle)
    expect(store.connectionStatus.value).toBe('closed')
    expect(store.error.value).toBeNull()
    expect(store.events.value).toHaveLength(1)
    expect(socket.close).toHaveBeenCalledOnce()
  })

  it('ignores the old snapshot after reset even when it resolves after a new run', async () => {
    const oldSnapshot = deferred<RunSnapshotDto>()
    const api = {
      start: vi.fn()
        .mockResolvedValueOnce({ run_id: 'run-old', events_url: '/events/old' })
        .mockResolvedValueOnce({ run_id: 'run-current', events_url: '/events/current' }),
      snapshot: vi.fn()
        .mockReturnValueOnce(oldSnapshot.promise)
        .mockResolvedValueOnce(snapshotFor('run-current')),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await store.submit('Old run')
    store.reset()
    await store.submit('Current run')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(1))
    oldSnapshot.resolve(snapshotFor('run-old', 'failed'))
    await oldSnapshot.promise
    await Promise.resolve()

    expect(store.runId.value).toBe('run-current')
    expect(store.view.value.runId).toBe('run-current')
    expect(store.lifecycle.value).toBe('running')
    expect(store.connectionStatus.value).toBe('connected')
    expect(sockets.sockets).toHaveLength(1)
  })

  it('ignores an old socket after reset even when the server reuses the same run id', async () => {
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-reused-id', events_url: '/events/reused-id' })),
      snapshot: vi.fn(async () => snapshotFor('run-reused-id')),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })
    await store.submit('Old session')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(1))
    const oldSocket = sockets.sockets[0]!
    store.reset()
    await store.submit('Current session')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(2))

    oldSocket.options.onStatus?.('fatal')
    oldSocket.options.onError?.(new Error('Old socket failed'))
    oldSocket.options.onEvent?.({
      ...fixtureEnvelope(1, { type: 'run_stopped', reason: 'failed' }),
      run_id: runId('run-reused-id'),
    })
    oldSocket.options.onResyncRequired?.({
      type: 'resync_required',
      run_id: runId('run-reused-id'),
      requested_after_sequence: 0,
      oldest_sequence: 1,
      latest_sequence: 1,
      reason: 'sequence_gap',
    })

    expect(store.lifecycle.value).toBe('running')
    expect(store.connectionStatus.value).toBe('connected')
    expect(store.error.value).toBeNull()
    expect(store.events.value).toHaveLength(0)
    expect(api.snapshot).toHaveBeenCalledTimes(2)
    expect(sockets.sockets).toHaveLength(2)
  })

  it('closes a live socket when an authoritative terminal snapshot replaces its state', async () => {
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-terminal-snapshot', events_url: '/events/terminal-snapshot' })),
      snapshot: vi.fn(async () => snapshotFor('run-terminal-snapshot')),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })
    await store.submit('Recover durable terminal state')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(1))
    const socket = sockets.sockets[0]!

    store.applySnapshot(snapshotFor('run-terminal-snapshot', 'succeeded'))
    socket.options.onEvent?.({
      ...fixtureEnvelope(1, { type: 'run_stopped', reason: 'failed' }),
      run_id: runId('run-terminal-snapshot'),
    })
    socket.options.onStatus?.('connected')
    socket.options.onError?.(new Error('Late socket failure'))

    expect(store.lifecycle.value).toBe('completed')
    expect(store.view.value.status).toBe('succeeded')
    expect(store.connectionStatus.value).toBe('closed')
    expect(store.events.value).toHaveLength(0)
    expect(store.error.value).toBeNull()
    expect(socket.close).toHaveBeenCalledOnce()
  })

  it('rejects a snapshot for a different run without replacing the accepted run', async () => {
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-current', events_url: '/events/current' })),
      snapshot: vi.fn(async () => snapshotFor('run-other', 'succeeded')),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await store.submit('Current run')
    await vi.waitFor(() => expect(store.error.value).toBeInstanceOf(RangeError))

    expect(store.runId.value).toBe('run-current')
    expect(store.lifecycle.value).toBe('running')
    expect(store.view.value.runId).toBeNull()
    expect(sockets.sockets).toHaveLength(0)
  })

  it('replaces a resyncing socket and ignores events from the superseded stream', async () => {
    const run = runId('run-resync')
    const initialSnapshot: RunSnapshotDto = {
      run_id: run,
      state: 'running',
      events: [{ ...fixtureEnvelope(1, { type: 'run_started', title: 'Resync run' }), run_id: run }],
      pending_approvals: [],
      oldest_sequence: 1,
      latest_sequence: 1,
      next_sequence: 2,
      updated_at: '2026-08-19T00:00:01.000Z',
    }
    const freshSnapshot: RunSnapshotDto = {
      ...initialSnapshot,
      events: [
        initialSnapshot.events[0]!,
        { ...fixtureEnvelope(2, { type: 'message', text: 'Recovered snapshot' }), run_id: run },
      ],
      latest_sequence: 2,
      next_sequence: 3,
      updated_at: '2026-08-19T00:00:02.000Z',
    }
    const api = {
      start: vi.fn(async () => ({ run_id: run, events_url: 'wss://runtime/events/resync' })),
      snapshot: vi
        .fn<() => Promise<RunSnapshotDto>>()
        .mockResolvedValueOnce(initialSnapshot)
        .mockResolvedValueOnce(freshSnapshot),
    } as unknown as RunsApi
    const sockets = createFakeRunSocketFactory()
    const store = createRunStore(api, { runSocketFactory: sockets.factory })

    await store.submit('Recover the stream')
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(1))
    sockets.sockets[0]?.options.onResyncRequired?.({
      type: 'resync_required',
      run_id: run,
      requested_after_sequence: 1,
      oldest_sequence: 1,
      latest_sequence: 2,
      reason: 'sequence_gap',
    })
    await vi.waitFor(() => expect(sockets.sockets).toHaveLength(2))

    expect(sockets.sockets[0]?.close).toHaveBeenCalledOnce()
    expect(sockets.sockets[1]?.options.afterSequence?.()).toBe(2)

    sockets.sockets[0]?.options.onEvent?.({
      ...fixtureEnvelope(3, { type: 'message', text: 'Stale transport event' }),
      run_id: run,
    })
    expect(store.events.value).toHaveLength(2)

    sockets.sockets[1]?.options.onEvent?.({
      ...fixtureEnvelope(3, { type: 'message', text: 'Current transport event' }),
      run_id: run,
    })
    expect(store.events.value.at(-1)?.kind).toMatchObject({
      type: 'message',
      text: 'Current transport event',
    })
  })

  it('cancels the active run and reaches a terminal lifecycle state', async () => {
    const cancel = vi.fn(async () => ({ run_id: 'run-cancel', stopped: true, usage: {} }))
    const api = { cancel } as unknown as RunsApi
    const store = createRunStore(api)
    store.runId.value = 'run-cancel'
    store.lifecycle.value = 'running'

    await expect(store.cancel()).resolves.toMatchObject({ stopped: true })
    expect(cancel).toHaveBeenCalledWith('run-cancel')
    expect(store.lifecycle.value).toBe('cancelled')
    expect(store.connectionStatus.value).toBe('closed')
  })

  it.each(['resolve', 'reject'] as const)('ignores a late cancellation %s after reset and a new run', async (settlement) => {
    const request = deferred<RunSummaryDto>()
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-current', events_url: '/events/current' })),
      cancel: vi.fn(() => request.promise),
    } as unknown as RunsApi
    const store = createRunStore(api)
    store.runId.value = 'run-old'
    store.lifecycle.value = 'running'

    const cancellation = store.cancel()
    store.reset()
    await store.submit('Current session')
    if (settlement === 'resolve') request.resolve({
      run_id: 'run-old',
      stopped: true,
      usage: { input_tokens: 0, output_tokens: 0, cached_input_tokens: 0, reasoning_output_tokens: 0 },
    })
    else request.reject(new Error('Late cancellation failure'))

    await expect(cancellation).resolves.toBeNull()
    expect(store.runId.value).toBe('run-current')
    expect(store.lifecycle.value).toBe('running')
    expect(store.error.value).toBeNull()
  })

  it('uses the durable outcome when cancellation races with successful completion', async () => {
    const api = {
      cancel: vi.fn(async () => ({ run_id: 'run-cancel-race', stopped: true, usage: {} })),
      snapshot: vi.fn(async () => snapshotFor('run-cancel-race', 'succeeded')),
    } as unknown as RunsApi
    const store = createRunStore(api)
    store.runId.value = 'run-cancel-race'
    store.lifecycle.value = 'running'

    await store.cancel()
    expect(store.lifecycle.value).toBe('completed')
    expect(store.view.value.status).toBe('succeeded')
    expect(store.connectionStatus.value).toBe('closed')
  })

  it.each(['resolve', 'reject'] as const)('ignores a late cancellation snapshot %s after reset', async (settlement) => {
    const snapshot = deferred<RunSnapshotDto>()
    const api = {
      start: vi.fn(async () => ({ run_id: 'run-current', events_url: '/events/current' })),
      cancel: vi.fn(async () => ({ run_id: 'run-old', stopped: true, usage: {} })),
      snapshot: vi.fn()
        .mockReturnValueOnce(snapshot.promise)
        .mockResolvedValueOnce(snapshotFor('run-current')),
    } as unknown as RunsApi
    const store = createRunStore(api)
    store.runId.value = 'run-old'
    store.lifecycle.value = 'running'
    const cancellation = store.cancel()
    await vi.waitFor(() => expect(api.snapshot).toHaveBeenCalledWith('run-old'))
    store.reset()
    await store.submit('Current session')
    await vi.waitFor(() => expect(store.view.value.runId).toBe('run-current'))

    if (settlement === 'resolve') snapshot.resolve(snapshotFor('run-old', 'cancelled'))
    else snapshot.reject(new Error('Late snapshot failed'))
    await expect(cancellation).resolves.toBeNull()
    expect(store.runId.value).toBe('run-current')
    expect(store.lifecycle.value).toBe('running')
    expect(store.view.value.status).toBe('running')
    expect(store.error.value).toBeNull()
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
