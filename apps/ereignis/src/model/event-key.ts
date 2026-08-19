import type { UiEventEnvelope, UiEventKind } from '@orchester/protokoll'

export type EventKey = string
export type TimelineKey = string

/** The journal identity used for sequence deduplication. */
export function eventKey(event: Pick<UiEventEnvelope, 'run_id' | 'sequence'>): EventKey {
  return `${event.run_id}:${event.sequence}`
}

/** A stable key for the rendered item represented by an event. */
export function timelineItemKey(event: UiEventEnvelope): TimelineKey {
  return timelineKindKey(event.kind, event.event_id, event.run_id, event.turn_id)
}

export function gapKey(runId: string, from: number, to: number): string {
  return `gap:${runId}:${from}-${to}`
}

/** Alias used by consumers that call rendered entries timeline keys. */
export const timelineKey = timelineItemKey

function timelineKindKey(
  kind: UiEventKind,
  eventId: string,
  runId: string,
  turnId: string | undefined,
): TimelineKey {
  switch (kind.type) {
    case 'run_started':
      return `run:${runId}`
    case 'turn_started':
      return `turn:${turnId ?? eventId}`
    case 'message':
      return `message:${eventId}`
    case 'message_delta':
      return `message-delta:${turnId ?? runId}`
    case 'reasoning':
      return `reasoning:${eventId}`
    case 'tool_call':
      return `tool:${kind.call_id}`
    case 'file_change':
      return `file-change:${eventId}`
    case 'todo_list':
      return `todo:${eventId}`
    case 'usage':
      return `usage:${eventId}`
    case 'approval_requested':
      return `approval:${kind.approval.approval_id}`
    case 'approval_resolved':
      return `approval:${kind.resolution.approval_id}`
    case 'validation':
      return `validation:${eventId}`
    case 'run_stopped':
      return `stop:${eventId}`
    case 'error':
      return `error:${eventId}`
    default:
      return assertNever(kind)
  }
}

function assertNever(value: never): never {
  throw new Error(`unhandled UI event kind: ${String(value)}`)
}
