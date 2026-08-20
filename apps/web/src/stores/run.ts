import {
  eventKey,
  projectRunEvents,
  projectRunSnapshot,
  type RunView,
} from '@orchester/ereignis'
import type {
  RunSnapshotDto,
  RunSummaryDto,
  StartRunResponse,
  UiEventEnvelope,
} from '@orchester/protokoll'
import { ref, shallowRef, type Ref } from 'vue'

import type { RunsApi, StartRunOptions } from '../api/runs'

export type RunProjectionStatus = 'idle' | 'ready' | 'gap' | 'error'
export type RunLifecycle =
  | 'idle'
  | 'submitting'
  | 'running'
  | 'cancelling'
  | 'completed'
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
  lifecycle: Ref<RunLifecycle>
  view: Readonly<Ref<RunView>>
  events: Readonly<Ref<readonly UiEventEnvelope[]>>
  projectionStatus: Readonly<Ref<RunProjectionStatus>>
  connectionStatus: Ref<RunConnectionStatus>
  error: Readonly<Ref<Error | null>>
  submit: (prompt: string) => Promise<StartRunResponse | null>
  cancel: () => Promise<RunSummaryDto | null>
  applySnapshot: (snapshot: RunSnapshotDto) => void
  applyEvent: (event: UiEventEnvelope) => boolean
  setConnectionStatus: (status: RunConnectionStatus) => void
  setError: (error: unknown) => void
  clearError: () => void
  reset: () => void
}

export interface RunStoreOptions {
  idempotencyKey?: () => string
}

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

export function createRunStore(api?: RunsApi, options: RunStoreOptions = {}): RunStore {
  const runId = ref<string | null>(null)
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

  function rebuild(): void {
    const ordered = [...journal.values()].sort((left, right) => left.sequence - right.sequence)
    events.value = ordered
    const options = headSequence === undefined ? { firstSequence } : { firstSequence, headSequence }
    view.value = projectRunEvents(ordered, options)
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : 'ready'
  }

  function applySnapshot(snapshot: RunSnapshotDto): void {
    projectedRunId = snapshot.run_id
    firstSequence = snapshot.oldest_sequence > 0 ? snapshot.oldest_sequence : 1
    headSequence = snapshot.latest_sequence
    journal.clear()
    for (const event of snapshot.events) {
      if (event.run_id !== snapshot.run_id) {
        throw new RangeError('snapshot event belongs to another run')
      }
      journal.set(eventKey(event), event)
    }
    view.value = projectRunSnapshot(snapshot)
    events.value = [...journal.values()].sort((left, right) => left.sequence - right.sequence)
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : 'ready'
    error.value = null
  }

  function applyEvent(event: UiEventEnvelope): boolean {
    if (projectedRunId !== null && event.run_id !== projectedRunId) {
      throw new RangeError('event belongs to another run')
    }
    projectedRunId ??= event.run_id
    if (event.sequence < firstSequence) return false
    const key = eventKey(event)
    if (journal.has(key)) return false
    journal.set(key, event)
    if (headSequence === undefined || event.sequence > headSequence) headSequence = event.sequence
    rebuild()
    error.value = null
    return true
  }

  function setError(cause: unknown): void {
    error.value = asError(cause)
    projectionStatus.value = 'error'
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

    lifecycle.value = 'submitting'
    connectionStatus.value = 'connecting'
    error.value = null
    try {
      const response = await api.start(
        { prompt: normalizedPrompt },
        requestOptions(makeIdempotencyKey()),
      )
      runId.value = response.run_id
      lifecycle.value = 'running'
      connectionStatus.value = 'connecting'
      return response
    } catch (cause) {
      lifecycle.value = 'failed'
      connectionStatus.value = 'error'
      setError(cause)
      return null
    }
  }

  async function cancel(): Promise<RunSummaryDto | null> {
    if (!api || !runId.value || lifecycle.value !== 'running') return null
    lifecycle.value = 'cancelling'
    try {
      const summary = await api.cancel(runId.value)
      lifecycle.value = 'completed'
      connectionStatus.value = 'closed'
      return summary
    } catch (cause) {
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
    runId.value = null
    lifecycle.value = 'idle'
    projectedRunId = null
    firstSequence = 1
    headSequence = undefined
    journal.clear()
    events.value = []
    view.value = projectRunEvents([])
    projectionStatus.value = 'idle'
    connectionStatus.value = 'idle'
    error.value = null
  }

  return {
    runId,
    lifecycle,
    view,
    events,
    projectionStatus,
    connectionStatus,
    error,
    submit,
    cancel,
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
