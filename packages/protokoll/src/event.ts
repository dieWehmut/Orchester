/**
 * The TypeScript mirror of `orchester_protokoll::Event`.
 *
 * Every field name here is `snake_case` on purpose. These types cross a serde
 * boundary, and serde is the authority: the Rust enum is tagged with
 * `#[serde(tag = "type", rename_all = "snake_case")]`, so the tag values and the
 * field names below are the wire format, not a style choice. Renaming one to
 * `camelCase` would compile and then silently fail to match anything.
 *
 * Source of truth: `kisten/protokoll/src/event.rs`, `result.rs`, `harness.rs`.
 */

/** Lifecycle status of a {@link ToolCallEvent}. */
export type ToolStatus = 'in_progress' | 'completed' | 'failed'

/** What happened to a file in a {@link FileChangeEvent}. */
export type ChangeKind = 'add' | 'update' | 'delete'

/**
 * Why a run stopped.
 *
 * Deliberately richer than success/failure: a governed run can also stop because
 * it is waiting for a human or because it ran out of budget, and a frontend has
 * to offer a different next step for each of those.
 */
export type StopReason =
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'awaiting_approval'
  | 'budget_exceeded'
  | 'repeated_failure'
  | 'interrupted_unknown_outcome'

/** One entry of the agent's running to-do list. */
export interface TodoItem {
  text: string
  completed: boolean
}

/** Token accounting. Cumulative across the run, not per turn. */
export interface Usage {
  input_tokens: number
  output_tokens: number
  cached_input_tokens: number
  reasoning_output_tokens: number
}

declare const approvalIdBrand: unique symbol

/**
 * An approval's identifier.
 *
 * A bare string on the wire — the Rust side is `#[serde(transparent)]` — but
 * branded here so that a session id or a run id cannot be handed to an approval
 * endpoint by mistake. Approving the wrong action is not a typo you want to make
 * silently, so constructing one requires {@link approvalId}.
 */
export type ApprovalId = string & { readonly [approvalIdBrand]: true }

/** Mark a raw string as an {@link ApprovalId}. */
export function approvalId(raw: string): ApprovalId {
  return raw as ApprovalId
}

export interface SessionStartedEvent {
  type: 'session_started'
  session_id: string
}

export interface TurnStartedEvent {
  type: 'turn_started'
}

/** Assistant natural-language output. */
export interface MessageEvent {
  type: 'message'
  text: string
}

/** The agent's reasoning digest, not its full chain of thought. */
export interface ReasoningEvent {
  type: 'reasoning'
  text: string
}

export interface ToolCallEvent {
  type: 'tool_call'
  name: string
  status: ToolStatus
  /** Absent rather than empty when the runtime has nothing safe to show. */
  detail?: string
}

export interface FileChangeEvent {
  type: 'file_change'
  path: string
  kind: ChangeKind
}

export interface TodoListEvent {
  type: 'todo_list'
  items: TodoItem[]
}

/**
 * Token usage.
 *
 * Note the shape: this is a serde *newtype* variant, so `Usage`'s fields are
 * flattened next to the tag rather than nested under a `usage` key. Pinned by
 * `usage_event_flattens_fields_alongside_tag` in
 * `kisten/protokoll/tests/roundtrip.rs`.
 */
export type UsageEvent = { type: 'usage' } & Usage

export interface TurnCompletedEvent {
  type: 'turn_completed'
}

/**
 * A governed action needs a human decision.
 *
 * `action` is the runtime's redacted one-line summary, never raw model output:
 * this event reaches a browser, and an approval prompt is exactly where a leaked
 * credential would be read and then approved.
 */
export interface ApprovalRequiredEvent {
  type: 'approval_required'
  approval_id: ApprovalId
  action: string
  reason: string
}

/** The final assistant message. */
export interface ResultEvent {
  type: 'result'
  text: string
}

export interface StoppedEvent {
  type: 'stopped'
  reason: StopReason
}

export interface ErrorEvent {
  type: 'error'
  message: string
}

/** One normalized event in an agent run. */
export type Event =
  | SessionStartedEvent
  | TurnStartedEvent
  | MessageEvent
  | ReasoningEvent
  | ToolCallEvent
  | FileChangeEvent
  | TodoListEvent
  | UsageEvent
  | TurnCompletedEvent
  | ApprovalRequiredEvent
  | ResultEvent
  | StoppedEvent
  | ErrorEvent

export type EventType = Event['type']

/**
 * Every tag the union covers.
 *
 * `satisfies` ties this list to the union in both directions: a tag that is not
 * an `EventType` fails to compile, and the test suite checks the reverse, that no
 * `EventType` is missing from the list. That second half is what catches a new
 * Rust variant arriving without a TypeScript counterpart.
 */
export const EVENT_TYPES = [
  'session_started',
  'turn_started',
  'message',
  'reasoning',
  'tool_call',
  'file_change',
  'todo_list',
  'usage',
  'turn_completed',
  'approval_required',
  'result',
  'stopped',
  'error',
] as const satisfies readonly EventType[]

export const TOOL_STATUSES = ['in_progress', 'completed', 'failed'] as const satisfies readonly ToolStatus[]

export const CHANGE_KINDS = ['add', 'update', 'delete'] as const satisfies readonly ChangeKind[]

export const STOP_REASONS = [
  'succeeded',
  'failed',
  'cancelled',
  'awaiting_approval',
  'budget_exceeded',
  'repeated_failure',
  'interrupted_unknown_outcome',
] as const satisfies readonly StopReason[]

/** The zero value, for a footer that must render before the first usage event. */
export const EMPTY_USAGE: Usage = {
  input_tokens: 0,
  output_tokens: 0,
  cached_input_tokens: 0,
  reasoning_output_tokens: 0,
}
