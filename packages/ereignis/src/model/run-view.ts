import type {
  ApprovalId,
  ChangeKind,
  StopReason,
  TodoItem,
  UiApprovalDecision,
  UiApprovalRequest,
  UiUsage,
  UiValidation,
  UiToolState,
  CallId,
  RunId,
  TurnId,
} from '@orchester/protokoll'

/** A view status derived from run events, never from a transport connection. */
export type RunStatus = 'idle' | 'running' | StopReason

export const RUN_STATUSES = [
  'idle',
  'running',
  'succeeded',
  'failed',
  'cancelled',
  'awaiting_approval',
  'budget_exceeded',
  'repeated_failure',
  'interrupted_unknown_outcome',
] as const satisfies readonly RunStatus[]

/** Stable translation keys; the host application owns localization. */
export const RUN_STATUS_LABEL_KEYS: Record<RunStatus, string> = {
  idle: 'run.status.idle',
  running: 'run.status.running',
  succeeded: 'run.status.succeeded',
  failed: 'run.status.failed',
  cancelled: 'run.status.cancelled',
  awaiting_approval: 'run.status.awaiting_approval',
  budget_exceeded: 'run.status.budget_exceeded',
  repeated_failure: 'run.status.repeated_failure',
  interrupted_unknown_outcome: 'run.status.interrupted_unknown_outcome',
}

export const TIMELINE_ITEM_TYPES = [
  'message',
  'reasoning',
  'tool',
  'file_change',
  'todo_list',
  'validation',
  'approval',
  'error',
  'gap',
] as const

export type TimelineItemType = (typeof TIMELINE_ITEM_TYPES)[number]

export const TIMELINE_LABEL_KEYS: Record<TimelineItemType, string> = {
  message: 'run.timeline.message',
  reasoning: 'run.timeline.reasoning',
  tool: 'run.timeline.tool',
  file_change: 'run.timeline.file_change',
  todo_list: 'run.timeline.todo_list',
  validation: 'run.timeline.validation',
  approval: 'run.timeline.approval',
  error: 'run.timeline.error',
  gap: 'run.timeline.gap',
}

export interface TimelineItemBase {
  readonly key: string
  readonly sequence: number
  readonly occurredAt: string
  readonly turnId: TurnId | null
}

export interface MessageTimelineItem extends TimelineItemBase {
  readonly type: 'message'
  readonly role: 'user' | 'assistant'
  readonly text: string
  readonly final: boolean
}

export interface ReasoningTimelineItem extends TimelineItemBase {
  readonly type: 'reasoning'
  readonly text: string
}

export interface ToolTimelineItem extends TimelineItemBase {
  readonly type: 'tool'
  readonly callId: CallId
  readonly name: string
  readonly state: UiToolState
  readonly detail: string | null
}

export interface FileChangeTimelineItem extends TimelineItemBase {
  readonly type: 'file_change'
  readonly path: string
  readonly kind: ChangeKind
}

export interface TodoListTimelineItem extends TimelineItemBase {
  readonly type: 'todo_list'
  readonly items: readonly Readonly<TodoItem>[]
}

export interface ValidationTimelineItem extends TimelineItemBase {
  readonly type: 'validation'
  readonly validation: Readonly<UiValidation>
}

export type ApprovalTimelineState = 'pending' | UiApprovalDecision

export interface ApprovalTimelineItem extends TimelineItemBase {
  readonly type: 'approval'
  readonly approvalId: ApprovalId
  readonly state: ApprovalTimelineState
  readonly request: Readonly<UiApprovalRequest> | null
  readonly decision: UiApprovalDecision | null
}

export interface ErrorTimelineItem extends TimelineItemBase {
  readonly type: 'error'
  readonly code: string
  readonly message: string
}

/** A synthetic timeline entry; gaps do not invent an event timestamp or turn. */
export interface GapTimelineItem {
  readonly type: 'gap'
  readonly key: string
  readonly sequence: number
  readonly occurredAt: null
  readonly turnId: null
  readonly missingFrom: number
  readonly missingTo: number
}

export type TimelineItem =
  | MessageTimelineItem
  | ReasoningTimelineItem
  | ToolTimelineItem
  | FileChangeTimelineItem
  | TodoListTimelineItem
  | ValidationTimelineItem
  | ApprovalTimelineItem
  | ErrorTimelineItem
  | GapTimelineItem

export interface TurnView {
  readonly key: string
  readonly id: TurnId | null
  readonly startedAt: string | null
  readonly endedAt: string | null
  readonly items: readonly TimelineItem[]
}

export interface ToolInvocationView {
  readonly key: string
  readonly callId: CallId
  readonly name: string
  readonly state: UiToolState
  readonly detail: string | null
  readonly firstSequence: number
  readonly lastSequence: number
  readonly history: readonly ToolTimelineItem[]
}

export interface ApprovalView {
  readonly key: string
  readonly approvalId: ApprovalId
  readonly runId: RunId
  readonly rowVersion: number
  readonly risk: string
  readonly action: string
  readonly reason: string
  readonly expiresAt: string | null
  readonly state: ApprovalTimelineState
  readonly requestedSequence: number | null
  readonly resolvedSequence: number | null
}

export interface RunStopView {
  readonly reason: StopReason
  readonly sequence: number
  readonly occurredAt: string
  readonly outcome: 'terminal' | 'paused' | 'unknown'
}

export interface SequenceGap {
  readonly key: string
  readonly from: number
  readonly to: number
}

export interface RunView {
  readonly runId: RunId | null
  readonly title: string | null
  readonly status: RunStatus
  readonly stop: RunStopView | null
  readonly turns: readonly TurnView[]
  readonly timeline: readonly TimelineItem[]
  readonly tools: readonly ToolInvocationView[]
  readonly approvals: readonly ApprovalView[]
  readonly usage: Readonly<UiUsage>
  readonly validation: Readonly<UiValidation> | null
  readonly todos: readonly Readonly<TodoItem>[]
  readonly fileChanges: readonly FileChangeTimelineItem[]
  readonly errors: readonly ErrorTimelineItem[]
  readonly latestSequence: number
  readonly bufferedSequences: readonly number[]
  readonly gaps: readonly SequenceGap[]
}

export function createEmptyRunView(runId: RunId | null = null): RunView {
  return {
    runId,
    title: null,
    status: 'idle',
    stop: null,
    turns: [],
    timeline: [],
    tools: [],
    approvals: [],
    usage: {
      input_tokens: 0,
      output_tokens: 0,
      cached_input_tokens: 0,
      reasoning_output_tokens: 0,
    },
    validation: null,
    todos: [],
    fileChanges: [],
    errors: [],
    latestSequence: 0,
    bufferedSequences: [],
    gaps: [],
  }
}
