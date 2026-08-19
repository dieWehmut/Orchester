import {
  eventKey,
  projectRunEvents,
  projectRunSnapshot,
  type RunView,
} from '@orchester/ereignis'
import type { RunSnapshotDto, UiEventEnvelope } from '@orchester/protokoll'
import { ref, shallowRef, type Ref } from 'vue'

export type RunProjectionStatus = 'idle' | 'ready' | 'gap' | 'error'
export type RunConnectionStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'offline'
  | 'closed'
  | 'error'

export interface RunStore {
  view: Readonly<Ref<RunView>>
  events: Readonly<Ref<readonly UiEventEnvelope[]>>
  projectionStatus: Readonly<Ref<RunProjectionStatus>>
  connectionStatus: Ref<RunConnectionStatus>
  error: Readonly<Ref<Error | null>>
  applySnapshot: (snapshot: RunSnapshotDto) => void
  applyEvent: (event: UiEventEnvelope) => boolean
  setConnectionStatus: (status: RunConnectionStatus) => void
  setError: (error: unknown) => void
  clearError: () => void
  reset: () => void
}

function asError(cause: unknown): Error {
  return cause instanceof Error ? cause : new Error(String(cause))
}

/**
 * Owns only the browser's durable event window. Network code feeds snapshots
 * and envelopes into this store; the deterministic projection remains in
 * `@orchester/ereignis` and can therefore be reused by the static website.
 */
export function createRunStore(): RunStore {
  const view = shallowRef<RunView>(projectRunEvents([]))
  const events = shallowRef<readonly UiEventEnvelope[]>([])
  const projectionStatus = ref<RunProjectionStatus>('idle')
  const connectionStatus = ref<RunConnectionStatus>('idle')
  const error = shallowRef<Error | null>(null)
  let runId: string | null = null
  let firstSequence = 1
  let headSequence: number | undefined
  const journal = new Map<string, UiEventEnvelope>()

  function rebuild(): void {
    const ordered = [...journal.values()].sort((left, right) => left.sequence - right.sequence)
    events.value = ordered
    const options = headSequence === undefined ? { firstSequence } : { firstSequence, headSequence }
    view.value = projectRunEvents(ordered, options)
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : 'ready'
  }

  function applySnapshot(snapshot: RunSnapshotDto): void {
    runId = snapshot.run_id
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
    if (runId !== null && event.run_id !== runId) {
      throw new RangeError('event belongs to another run')
    }
    runId ??= event.run_id
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

  function clearError(): void {
    error.value = null
    projectionStatus.value = view.value.gaps.length > 0 ? 'gap' : view.value.runId ? 'ready' : 'idle'
  }

  function reset(): void {
    runId = null
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
    view,
    events,
    projectionStatus,
    connectionStatus,
    error,
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
