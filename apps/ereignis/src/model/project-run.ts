import type {
  RunId,
  RunSnapshotDto,
  UiEventEnvelope,
} from '@orchester/protokoll'

import { eventKey, gapKey } from './event-key'
import {
  createEmptyRunView,
  type GapTimelineItem,
  type RunStatus,
  type RunView,
  type SequenceGap,
  type RunStopView,
} from './run-view'

/** Optional sequence bounds used when projecting a bounded journal snapshot. */
export interface ProjectRunEventsOptions {
  /** The first sequence represented by the input journal. Defaults to one. */
  readonly firstSequence?: number
  /** The journal head, used to expose a trailing gap in a bounded snapshot. */
  readonly headSequence?: number
}

/**
 * Project one event collection into a deterministic run view.
 *
 * Events are keyed by `(run_id, sequence)`, so replaying a sequence with a
 * different event id or payload does not replace the first observed event.
 * Only the contiguous prefix is applied; events after the first hole remain
 * buffered and the missing ranges are represented by synthetic timeline items.
 */
export function projectRunEvents(
  events: readonly UiEventEnvelope[],
  options: ProjectRunEventsOptions = {},
): RunView {
  return projectEventCollection(events, undefined, options)
}

/**
 * Replace the current projection with a server run snapshot.
 *
 * This function is intentionally stateless: callers pass the snapshot they
 * want rendered, and events from any prior projection cannot leak into it.
 */
export function projectRunSnapshot(snapshot: RunSnapshotDto): RunView {
  assertSnapshotMetadata(snapshot)
  const view = projectEventCollection(snapshot.events, snapshot.run_id, {
    firstSequence: snapshot.oldest_sequence > 0 ? snapshot.oldest_sequence : 1,
    headSequence: snapshot.latest_sequence,
  })

  return {
    ...view,
    // The snapshot state is authoritative even when its bounded event window
    // does not include the corresponding lifecycle event.
    status: snapshotStateToStatus(snapshot.state),
  }
}

/** Alias for consumers that name the operation after the snapshot endpoint. */
export const projectSnapshot = projectRunSnapshot

function projectEventCollection(
  events: readonly UiEventEnvelope[],
  runIdHint: RunId | undefined,
  options: ProjectRunEventsOptions,
): RunView {
  const firstSequence = options.firstSequence ?? 1
  assertSequence(firstSequence, 'first sequence')
  if (options.headSequence !== undefined) {
    assertSequence(options.headSequence, 'head sequence', false)
    if (options.headSequence < firstSequence - 1) {
      throw new RangeError('head sequence cannot precede the first sequence')
    }
  }

  if (events.length === 0) {
    const empty = createEmptyRunView(runIdHint ?? null)
    const headSequence = options.headSequence
    if (headSequence === undefined || headSequence < firstSequence) return empty

    const gaps = collectSequenceGaps(
      runIdHint,
      firstSequence - 1,
      [],
      headSequence,
    )
    return withSequenceState(empty, firstSequence - 1, [], gaps)
  }

  const runId = runIdHint ?? events[0]!.run_id
  const byKey = new Map<string, UiEventEnvelope>()
  for (const event of events) {
    assertProjectableEvent(event, runId)
    const key = eventKey(event)
    if (!byKey.has(key)) byKey.set(key, event)
  }

  const orderedEvents = [...byKey.values()].sort(
    (left, right) => left.sequence - right.sequence,
  )
  const lastObservedSequence = orderedEvents.at(-1)!.sequence
  if (options.headSequence !== undefined && lastObservedSequence > options.headSequence) {
    throw new RangeError('event sequence cannot exceed snapshot head')
  }
  if (orderedEvents[0]!.sequence < firstSequence) {
    throw new RangeError('event sequence cannot precede snapshot window')
  }

  const appliedEvents: UiEventEnvelope[] = []
  let nextSequence = firstSequence
  for (const event of orderedEvents) {
    if (event.sequence < nextSequence) continue
    if (event.sequence !== nextSequence) break
    appliedEvents.push(event)
    nextSequence += 1
  }

  const latestSequence = nextSequence - 1
  const bufferedSequences = orderedEvents
    .filter((event) => event.sequence > latestSequence)
    .map((event) => event.sequence)
  const gaps = collectSequenceGaps(
    runId,
    latestSequence,
    bufferedSequences,
    options.headSequence,
  )
  const empty = createEmptyRunView(runId)
  const projected = projectLifecycle(empty, appliedEvents)
  return withSequenceState(projected, latestSequence, bufferedSequences, gaps)
}

function withSequenceState(
  view: RunView,
  latestSequence: number,
  bufferedSequences: readonly number[],
  gaps: readonly SequenceGap[],
): RunView {
  return {
    ...view,
    timeline: gaps.map(toGapTimelineItem),
    latestSequence,
    bufferedSequences: [...bufferedSequences],
    gaps: [...gaps],
  }
}

function projectLifecycle(
  view: RunView,
  events: readonly UiEventEnvelope[],
): RunView {
  let title = view.title
  let status: RunStatus = view.status
  let stop = view.stop

  for (const event of events) {
    switch (event.kind.type) {
      case 'run_started':
        title ??= event.kind.title ?? null
        status = 'running'
        break
      case 'run_stopped':
        status = event.kind.reason
        stop = toRunStop(event)
        break
      default:
        break
    }
  }

  return { ...view, title, status, stop }
}

function toRunStop(event: UiEventEnvelope): RunStopView {
  if (event.kind.type !== 'run_stopped') {
    throw new TypeError('run stop view requires a run_stopped event')
  }
  const reason = event.kind.reason
  const outcome: RunStopView['outcome'] =
    reason === 'succeeded' || reason === 'failed' || reason === 'cancelled'
      ? 'terminal'
      : reason === 'interrupted_unknown_outcome'
        ? 'unknown'
        : 'paused'
  return {
    reason,
    sequence: event.sequence,
    occurredAt: event.occurred_at,
    outcome,
  }
}

function assertProjectableEvent(event: UiEventEnvelope, runId: RunId): void {
  if (event.run_id !== runId) {
    throw new RangeError('cannot project events from more than one run')
  }
  assertSequence(event.sequence, 'event sequence')
}

function assertSequence(value: number, label: string, positive = true): void {
  if (
    !Number.isSafeInteger(value) ||
    typeof value !== 'number' ||
    (positive ? value <= 0 : value < 0)
  ) {
    throw new RangeError(`${label} must be a ${positive ? 'positive ' : ''}safe integer`)
  }
}

function assertSnapshotMetadata(snapshot: RunSnapshotDto): void {
  assertSequence(snapshot.oldest_sequence, 'snapshot oldest sequence', false)
  assertSequence(snapshot.latest_sequence, 'snapshot latest sequence', false)
  assertSequence(snapshot.next_sequence, 'snapshot next sequence')
  if (snapshot.next_sequence !== snapshot.latest_sequence + 1) {
    throw new RangeError('snapshot next sequence must follow latest sequence')
  }
  if (
    snapshot.latest_sequence > 0 &&
    snapshot.oldest_sequence > snapshot.latest_sequence
  ) {
    throw new RangeError('snapshot oldest sequence cannot exceed latest sequence')
  }
}

function collectSequenceGaps(
  runId: RunId | undefined,
  latestSequence: number,
  bufferedSequences: readonly number[],
  headSequence: number | undefined,
): SequenceGap[] {
  if (runId === undefined) return []

  const gaps: SequenceGap[] = []
  let missingFrom = latestSequence + 1
  for (const sequence of bufferedSequences) {
    if (sequence > missingFrom) {
      gaps.push(makeGap(runId, missingFrom, sequence - 1))
    }
    missingFrom = sequence + 1
  }
  if (headSequence !== undefined && missingFrom <= headSequence) {
    gaps.push(makeGap(runId, missingFrom, headSequence))
  }
  return gaps
}

function makeGap(runId: RunId, from: number, to: number): SequenceGap {
  return {
    key: gapKey(runId, from, to),
    from,
    to,
  }
}

function toGapTimelineItem(gap: SequenceGap): GapTimelineItem {
  return {
    type: 'gap',
    key: gap.key,
    sequence: gap.from,
    occurredAt: null,
    turnId: null,
    missingFrom: gap.from,
    missingTo: gap.to,
  }
}

function snapshotStateToStatus(state: RunSnapshotDto['state']): RunStatus {
  switch (state) {
    case 'created':
      return 'idle'
    case 'validating':
      return 'running'
    case 'paused':
      return 'interrupted_unknown_outcome'
    default:
      return state
  }
}
