/**
 * What the shell says out loud, and when.
 *
 * Section 7 splits announcements three ways: run status, approval arrival and
 * validator results are `polite`; failures are `assertive`; and streaming
 * tokens are not announced at all - the turn is announced once, when it
 * completes.
 *
 * The last rule is the reason this is a function over *triggers* rather than a
 * live region attached to the transcript. A region on the transcript would read
 * every token as it arrived, because a token is a text change like any other.
 * Here the caller has to name the change it observed, and a streamed token is
 * not one of the names: only the end of a turn is.
 */

import type { RunStatus } from '@orchester/ereignis'
import type { UiEventEnvelope, UiValidation } from '@orchester/protokoll'

export type AnnouncementPoliteness = 'polite' | 'assertive'
export type AnnouncementKind = 'status' | 'turn' | 'approval' | 'validation'

/** The copy the host application keeps for each thing worth saying. */
export type AnnouncementKey =
  | 'run.announce.started'
  | 'run.announce.succeeded'
  | 'run.announce.failed'
  | 'run.announce.cancelled'
  | 'run.announce.budgetExceeded'
  | 'run.announce.interrupted'
  | 'run.announce.approval'
  | 'run.announce.turnComplete'
  | 'run.announce.validation'
  | 'run.announce.validationFailed'

export interface Announcement {
  kind: AnnouncementKind
  politeness: AnnouncementPoliteness
  /** A stable translation key; the host application owns the copy. */
  key: AnnouncementKey
  /** Values for the key's placeholders, when it carries any. */
  params?: Record<string, string>
}

/** The change the caller observed. A streamed token is deliberately not one. */
export type AnnouncementTrigger =
  | { kind: 'status'; status: RunStatus; previousStatus: RunStatus }
  | { kind: 'turn'; turnId: string | null }
  | { kind: 'approval' }
  | { kind: 'validation'; validation: UiValidation }

/** The politeness each announcement kind carries at its calm default. */
export const ANNOUNCEMENT_POLITENESS: Record<AnnouncementKind, AnnouncementPoliteness> = {
  status: 'polite',
  turn: 'polite',
  approval: 'polite',
  validation: 'polite',
}

/** Reasons a reader has to be told about at once, not when they are free. */
const FAILURE_STATUSES: readonly RunStatus[] = [
  'failed',
  'repeated_failure',
  'budget_exceeded',
  // The outcome is unknown, so the reader's picture of the run is wrong until
  // they are told: that is exactly what interrupting is for.
  'interrupted_unknown_outcome',
]

/**
 * The statuses worth a sentence. `awaiting_approval` is deliberately absent:
 * the approval arrival is the news, and the status that follows it would say
 * the same thing twice.
 */
const STATUS_KEYS: Partial<Record<RunStatus, AnnouncementKey>> = {
  running: 'run.announce.started',
  succeeded: 'run.announce.succeeded',
  failed: 'run.announce.failed',
  cancelled: 'run.announce.cancelled',
  budget_exceeded: 'run.announce.budgetExceeded',
  repeated_failure: 'run.announce.failed',
  interrupted_unknown_outcome: 'run.announce.interrupted',
}

/**
 * The one thing worth saying for a trigger, or `null` when the trigger carries
 * no news. A terminal failure is the only thing that interrupts: everything
 * else waits for the reader to finish their sentence.
 */
export function announcementFor(trigger: AnnouncementTrigger): Announcement | null {
  switch (trigger.kind) {
    case 'status': {
      const key = STATUS_KEYS[trigger.status]
      if (key === undefined || trigger.status === trigger.previousStatus) return null
      return {
        kind: 'status',
        politeness: FAILURE_STATUSES.includes(trigger.status) ? 'assertive' : 'polite',
        key,
      }
    }
    case 'approval':
      return { kind: 'approval', politeness: 'polite', key: 'run.announce.approval' }
    case 'turn':
      return { kind: 'turn', politeness: 'polite', key: 'run.announce.turnComplete' }
    case 'validation':
      // The result is the summary: "validation ran" without it tells the reader
      // nothing they can act on.
      return trigger.validation.ok
        ? {
            kind: 'validation',
            politeness: 'polite',
            key: 'run.announce.validation',
            params: { summary: trigger.validation.summary },
          }
        : {
            kind: 'validation',
            politeness: 'assertive',
            key: 'run.announce.validationFailed',
            params: { summary: trigger.validation.summary },
          }
  }
}

/**
 * How far the reader has been told: the run, the last sequence considered, the
 * status the run was last in, and the turns already announced. The turn set is
 * what keeps a reply from speaking twice when it streams to a final chunk and
 * then also lands as a whole message.
 */
export interface AnnouncementCursor {
  readonly runId: string | null
  readonly sequence: number
  readonly status: RunStatus
  readonly announcedTurns: ReadonlySet<string>
}

/** The cursor before any journal has been seen. */
export const EMPTY_ANNOUNCEMENT_CURSOR: AnnouncementCursor = {
  runId: null,
  sequence: 0,
  status: 'idle',
  announcedTurns: new Set(),
}

/**
 * The cursor for a reader who has just arrived: everything in the journal has
 * already happened, so none of it is news.
 */
export function cursorAtHead(events: readonly UiEventEnvelope[]): AnnouncementCursor {
  const ordered = orderBySequence(events)
  let status: RunStatus = 'idle'
  for (const event of ordered) {
    if (event.kind.type === 'run_started') status = 'running'
    else if (event.kind.type === 'run_stopped') status = event.kind.reason
  }
  return {
    runId: ordered.at(-1)?.run_id ?? null,
    sequence: ordered.at(-1)?.sequence ?? 0,
    status,
    announcedTurns: new Set(),
  }
}

/**
 * The announcements the journal has to offer beyond the cursor, in order.
 *
 * A journal that belongs to another run starts over, because a new run is news
 * from its first event. A journal that rewound below the cursor - the server
 * replaced it with a snapshot - moves the cursor to where the reader already is
 * instead of replaying the retained window out loud.
 */
export function advanceAnnouncements(
  cursor: AnnouncementCursor,
  events: readonly UiEventEnvelope[],
): { cursor: AnnouncementCursor; announcements: readonly Announcement[] } {
  const ordered = orderBySequence(events)
  const latest = ordered.at(-1)
  let active = cursor
  if (latest !== undefined && latest.run_id !== cursor.runId) {
    active = { runId: latest.run_id, sequence: 0, status: 'idle', announcedTurns: new Set() }
  }
  const head = latest?.sequence ?? active.sequence
  const since = head < active.sequence ? head : active.sequence

  const announcements: Announcement[] = []
  const announcedTurns = new Set(active.announcedTurns)
  let status = active.status

  for (const event of ordered) {
    if (event.sequence <= since) continue

    let trigger: AnnouncementTrigger | null = null
    switch (event.kind.type) {
      case 'run_started':
        trigger = { kind: 'status', status: 'running', previousStatus: status }
        status = 'running'
        break
      case 'run_stopped':
        trigger = { kind: 'status', status: event.kind.reason, previousStatus: status }
        status = event.kind.reason
        break
      case 'message_delta':
        // The intermediate chunks are text, not news; only the final chunk
        // ends the turn.
        if (event.kind.final) trigger = { kind: 'turn', turnId: event.turn_id ?? null }
        break
      case 'message':
        trigger = { kind: 'turn', turnId: event.turn_id ?? null }
        break
      case 'approval_requested':
        trigger = { kind: 'approval' }
        break
      case 'validation':
        trigger = { kind: 'validation', validation: event.kind.validation }
        break
      default:
        break
    }
    if (trigger === null) continue

    if (trigger.kind === 'turn') {
      // Without a turn id two events cannot be proven to be the same turn, so
      // the event's own sequence stands in: the worst case is one extra
      // sentence, never a swallowed turn.
      const turnKey = trigger.turnId ?? `event:${event.sequence}`
      if (announcedTurns.has(turnKey)) continue
      announcedTurns.add(turnKey)
    }

    const announcement = announcementFor(trigger)
    if (announcement !== null) announcements.push(announcement)
  }

  return {
    cursor: { runId: active.runId, sequence: head, status, announcedTurns },
    announcements,
  }
}

function orderBySequence(events: readonly UiEventEnvelope[]): UiEventEnvelope[] {
  return [...events].sort((left, right) => left.sequence - right.sequence)
}
