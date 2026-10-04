import {
  eventKey,
  projectRunEvents,
  projectRunSnapshot,
  type RunView,
} from '@orchester/ereignis'
import type {
  RunSnapshotDto,
  RunStateDto,
  RunSummaryDto,
  StartRunResponse,
  StopReason,
  UiEventEnvelope,
} from '@orchester/protokoll'
import { ref, shallowRef, type Ref } from 'vue'

import type { RunsApi, StartRunOptions } from '../api/runs'
import {
  type RunSocket,
  type RunSocketOptions,
  type RunSocketStatus,
} from '../transport/run-socket'

export type RunProjectionStatus = 'idle' | 'ready' | 'gap' | 'error'
export type RunLifecycle =
  | 'idle'
  | 'submitting'
  | 'running'
  | 'cancelling'
  | 'completed'
  | 'cancelled'
  | 'paused'
  | 'failed'
export type RunConnectionStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'offline'
  | 'closed'
  | 'error'

export interface RunStore {
  runId: Ref<string | null>
  /** True once the user submitted a prompt or the server delivered a run event. */
  conversationStarted: Ref<boolean>
  lifecycle: Ref<RunLifecycle>
  view: Readonly<Ref<RunView>>
  events: Readonly<Ref<readonly UiEventEnvelope[]>>
  projectionStatus: Readonly<Ref<RunProjectionStatus>>
  connectionStatus: Ref<RunConnectionStatus>
  error: Readonly<Ref<Error | null>>
  submit: (prompt: string) => Promise<StartRunResponse | null>
  cancel: () => Promise<RunSummaryDto | null>
  stop: () => void
  applySnapshot: (snapshot: RunSnapshotDto) => void
  applyEvent: (event: UiEventEnvelope) => boolean
  setConnectionStatus: (status: RunConnectionStatus) => void
  setError: (error: unknown) => void
  clearError: () => void
  reset: () => void
}

export interface RunStoreOptions {
  idempotencyKey?: () => string
  runSocketFactory?: RunSocketFactory
}

export type RunSocketFactory = (options: RunSocketOptions) => RunSocket

function asError(cause: unknown): Error {
  return cause instanceof Error ? cause : new Error(String(cause))
}

/**
 * Owns only the browser's durable event window. Network code feeds snapshots
 * and envelopes into this store; the deterministic projection remains in
 * `@orchester/ereignis` and can therefore be reused by the static website.
 */
function defaultIdempotencyKey(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID()
  return `run-request-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}

function requestOptions(idempotencyKey: string): StartRunOptions {
  return { idempotencyKey }
}

function lifecycleFromState(state: RunStateDto): RunLifecycle {
  if (state === 'succeeded') return 'completed'
  if (state === 'failed' || state === 'cancelled' || state === 'paused') return state
  return 'running'
}

function lifecycleFromStopReason(reason: StopReason): RunLifecycle {
  if (reason === 'succeeded') return 'completed'
  if (reason === 'cancelled') return 'cancelled'
  return 'failed'
}

function isTerminal(lifecycle: RunLifecycle): boolean {
  return lifecycle === 'completed' || lifecycle === 'failed' || lifecycle === 'cancelled' || lifecycle === 'paused'
}

export function createRunStore(api?: RunsApi, options: RunStoreOptions = {}): RunStore {
  const runId = ref<string | null>(null)
  const conversationStarted = ref(false)
  const lifecycle = ref<RunLifecycle>('idle')
  const view = shallowRef<RunView>(projectRunEvents([]))
  const events = shallowRef<readonly UiEventEnvelope[]>([])
  const projectionStatus = ref<RunProjectionStatus>('idle')
  const connectionStatus = ref<RunConnectionStatus>('idle')
  const error = shallowRef<Error | null>(null)
  let projectedRunId: string | null = null
  let firstSequence = 1
  let headSequence: number | undefined
  const journal = new Map<string, UiEventEnvelope>()
  const makeIdempotencyKey = options.idempotencyKey ?? defaultIdempotencyKey
  const runSocketFactory = options.runSocketFactory
  let activeSocket: RunSocket | null = null
  let streamEpoch = 0
  let operationEpoch = 0
  // Until start acknowledges the intent, a lost response must be recoverable
  // with the same key. Acknowledged runs and reset sessions start fresh intents.
  let pendingSubmission: { prompt: string; idempotencyKey: string } | null = null

  function clearProjection(): void {
    projectedRunId = null
    firstSequence = 1
    headSequence = undefined
    journal.clear()
    events.value = []
    view.value = projectRunEvents([])
    projectionStatus.value = 'idle'
  }

  function rebuild(): void {
    const ordered = [...journal.values()].sort((left, right) => left.sequence - right.sequence)
    events.value = ordered
    const options = headSequence === undefined ? { firstSequence } : { firstSequence, headSequence }
    view.value = projectRunEvents(ordered, options)
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : 'ready'
  }

  function applySnapshot(snapshot: RunSnapshotDto): void {
    const projected = projectRunSnapshot(snapshot)
    runId.value = snapshot.run_id
    projectedRunId = snapshot.run_id
    conversationStarted.value = true
    firstSequence = snapshot.oldest_sequence > 0 ? snapshot.oldest_sequence : 1
    headSequence = snapshot.latest_sequence
    journal.clear()
    for (const event of snapshot.events) {
      if (event.run_id !== snapshot.run_id) {
        throw new RangeError('snapshot event belongs to another run')
      }
      journal.set(eventKey(event), event)
    }
    view.value = projected
    events.value = [...journal.values()].sort((left, right) => left.sequence - right.sequence)
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : 'ready'
    error.value = null
    lifecycle.value = lifecycleFromState(snapshot.state)
    if (isTerminal(lifecycle.value)) {
      stop()
      connectionStatus.value = 'closed'
    }
  }

  function applyEvent(event: UiEventEnvelope): boolean {
    if (projectedRunId !== null && event.run_id !== projectedRunId) {
      throw new RangeError('event belongs to another run')
    }
    projectedRunId ??= event.run_id
    conversationStarted.value = true
    if (event.sequence < firstSequence) return false
    const key = eventKey(event)
    if (journal.has(key)) return false
    journal.set(key, event)
    if (headSequence === undefined || event.sequence > headSequence) headSequence = event.sequence
    rebuild()
    error.value = null
    if (event.kind.type === 'run_stopped') {
      lifecycle.value = lifecycleFromStopReason(event.kind.reason)
    }
    return true
  }

  function setError(cause: unknown): void {
    error.value = asError(cause)
    projectionStatus.value = 'error'
  }

  function setSocketStatus(status: RunSocketStatus): void {
    const mapped: RunConnectionStatus = status === 'fatal' ? 'error' : status
    connectionStatus.value = mapped
  }

  function stop(): void {
    operationEpoch += 1
    streamEpoch += 1
    activeSocket?.close()
    activeSocket = null
  }

  async function attachRunStream(
    response: StartRunResponse,
    epoch: number,
    hydratedSnapshot?: RunSnapshotDto,
  ): Promise<void> {
    if (!api || typeof api.snapshot !== 'function') return
    const expectedRunId = response.run_id
    try {
      const snapshot = hydratedSnapshot ?? (await api.snapshot(expectedRunId))
      if (streamEpoch !== epoch || runId.value !== expectedRunId) return
      if (snapshot.run_id !== expectedRunId) throw new RangeError('snapshot belongs to another run')
      applySnapshot(snapshot)
      if (isTerminal(lifecycle.value) || !runSocketFactory) return

      const socket = runSocketFactory({
        ticketProvider: () => response.events_url,
        afterSequence: () => events.value.at(-1)?.sequence ?? 0,
        onEvent: (event) => {
          if (streamEpoch !== epoch || runId.value !== expectedRunId) return
          try {
            if (applyEvent(event) && event.kind.type === 'run_stopped') {
              stop()
              connectionStatus.value = 'closed'
            }
          } catch (cause) {
            setError(cause)
          }
        },
        onResyncRequired: () => {
          if (streamEpoch !== epoch || runId.value !== expectedRunId) return
          const nextEpoch = epoch + 1
          streamEpoch = nextEpoch
          activeSocket?.close()
          activeSocket = null
          void attachRunStream(response, nextEpoch).catch((cause) => {
            if (streamEpoch === nextEpoch && runId.value === expectedRunId) {
              connectionStatus.value = 'error'
              setError(cause)
            }
          })
        },
        onError: (cause) => {
          if (streamEpoch === epoch && runId.value === expectedRunId) setError(cause)
        },
        onStatus: (status) => {
          if (streamEpoch === epoch && runId.value === expectedRunId) setSocketStatus(status)
        },
      })
      if (streamEpoch !== epoch || runId.value !== expectedRunId) {
        socket.close()
        return
      }
      activeSocket = socket
      await socket.connect()
      if (streamEpoch !== epoch || runId.value !== expectedRunId) {
        socket.close()
        if (activeSocket === socket) activeSocket = null
      }
    } catch (cause) {
      if (streamEpoch !== epoch || runId.value !== expectedRunId) return
      connectionStatus.value = 'error'
      setError(cause)
    }
  }

  async function submit(prompt: string): Promise<StartRunResponse | null> {
    const normalizedPrompt = prompt.trim()
    if (!normalizedPrompt || lifecycle.value === 'submitting' || lifecycle.value === 'running' || lifecycle.value === 'cancelling') {
      return null
    }
    if (!api) {
      lifecycle.value = 'failed'
      connectionStatus.value = 'error'
      setError(new Error('Run service is unavailable'))
      return null
    }

    if (pendingSubmission?.prompt !== normalizedPrompt) {
      pendingSubmission = { prompt: normalizedPrompt, idempotencyKey: makeIdempotencyKey() }
    }
    const submission = pendingSubmission
    stop()
    const epoch = operationEpoch
    lifecycle.value = 'submitting'
    conversationStarted.value = true
    connectionStatus.value = 'connecting'
    clearError()
    const streamEpochForRun = streamEpoch
    try {
      const response = await api.start(
        { prompt: normalizedPrompt },
        requestOptions(submission.idempotencyKey),
      )
      if (operationEpoch !== epoch) return null
      pendingSubmission = null
      clearProjection()
      runId.value = response.run_id
      lifecycle.value = 'running'
      connectionStatus.value = 'connecting'
      void attachRunStream(response, streamEpochForRun)
      return response
    } catch (cause) {
      if (operationEpoch !== epoch) return null
      lifecycle.value = 'failed'
      connectionStatus.value = 'error'
      setError(cause)
      return null
    }
  }

  async function cancel(): Promise<RunSummaryDto | null> {
    if (!api || !runId.value || lifecycle.value !== 'running') return null
    const expectedRunId = runId.value
    lifecycle.value = 'cancelling'
    stop()
    const epoch = operationEpoch
    try {
      const summary = await api.cancel(expectedRunId)
      if (operationEpoch !== epoch || runId.value !== expectedRunId) return null
      lifecycle.value = 'cancelled'
      connectionStatus.value = 'closed'
      // Cancellation can race with successful completion. The durable state,
      // when available, tells the footer which terminal outcome actually won.
      if (typeof api.snapshot === 'function') {
        try {
          const snapshot = await api.snapshot(expectedRunId)
          if (operationEpoch !== epoch || runId.value !== expectedRunId) return null
          if (snapshot.run_id !== expectedRunId) throw new RangeError('snapshot belongs to another run')
          applySnapshot(snapshot)
        } catch (cause) {
          if (operationEpoch !== epoch || runId.value !== expectedRunId) return null
          setError(cause)
        }
      }
      return summary
    } catch (cause) {
      if (operationEpoch !== epoch || runId.value !== expectedRunId) return null
      lifecycle.value = 'failed'
      connectionStatus.value = 'error'
      setError(cause)
      return null
    }
  }

  function clearError(): void {
    error.value = null
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : view.value.runId ? 'ready' : 'idle'
  }

  function reset(): void {
    stop()
    runId.value = null
    conversationStarted.value = false
    lifecycle.value = 'idle'
    pendingSubmission = null
    clearProjection()
    connectionStatus.value = 'idle'
    error.value = null
  }

  return {
    runId,
    conversationStarted,
    lifecycle,
    view,
    events,
    projectionStatus,
    connectionStatus,
    error,
    submit,
    cancel,
    stop,
    applySnapshot,
    applyEvent,
    setConnectionStatus: (status) => {
      connectionStatus.value = status
    },
    setError,
    clearError,
    reset,
  }
}
