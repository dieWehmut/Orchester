/**
 * TypeScript mirror of the redaction-safe browser UI envelope in
 * `kisten/protokoll/src/ui.rs`.
 *
 * This module only describes the wire shape. Runtime validation belongs in
 * `ui-guards.ts`, so static website fixtures can use the same types without
 * importing a transport or application store.
 */

import type { ApprovalId, ChangeKind, StopReason, TodoItem } from './event'

export const UI_SCHEMA_VERSION = 1 as const
export const LEGACY_EVENT_SCHEMA_VERSION = 0 as const

declare const eventIdBrand: unique symbol
declare const runIdBrand: unique symbol
declare const turnIdBrand: unique symbol
declare const callIdBrand: unique symbol

export type EventId = string & { readonly [eventIdBrand]: true }
export type RunId = string & { readonly [runIdBrand]: true }
export type TurnId = string & { readonly [turnIdBrand]: true }
export type CallId = string & { readonly [callIdBrand]: true }

export const eventId = (raw: string): EventId => raw as EventId
export const runId = (raw: string): RunId => raw as RunId
export const turnId = (raw: string): TurnId => raw as TurnId
export const callId = (raw: string): CallId => raw as CallId

export type UiToolState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled'
export const UI_TOOL_STATES = [
  'queued',
  'running',
  'succeeded',
  'failed',
  'cancelled',
] as const satisfies readonly UiToolState[]

export type UiApprovalDecision = 'approved' | 'denied' | 'expired' | 'stale'
export const UI_APPROVAL_DECISIONS = ['approved', 'denied', 'expired', 'stale'] as const satisfies readonly UiApprovalDecision[]

export interface UiUsage {
  input_tokens: number
  output_tokens: number
  cached_input_tokens: number
  reasoning_output_tokens: number
}

export interface UiApprovalRequest {
  approval_id: ApprovalId
  run_id: RunId
  row_version: number
  risk: string
  action: string
  reason: string
  expires_at?: string
}

export interface UiApprovalResolution {
  approval_id: ApprovalId
  row_version: number
  decision: UiApprovalDecision
}

export interface UiValidation {
  ok: boolean
  summary: string
  details?: string
}

export interface RunStartedUiEvent {
  type: 'run_started'
  title?: string
}

export interface TurnStartedUiEvent {
  type: 'turn_started'
}

export interface MessageUiEvent {
  type: 'message'
  text: string
}

export interface MessageDeltaUiEvent {
  type: 'message_delta'
  text: string
  final: boolean
}

export interface ReasoningUiEvent {
  type: 'reasoning'
  text: string
}

export interface ToolCallUiEvent {
  type: 'tool_call'
  call_id: CallId
  name: string
  state: UiToolState
  detail?: string
}

export interface FileChangeUiEvent {
  type: 'file_change'
  path: string
  kind: ChangeKind
}

export interface TodoListUiEvent {
  type: 'todo_list'
  items: TodoItem[]
}

export interface UsageUiEvent extends UiUsage {
  type: 'usage'
}

export interface ApprovalRequestedUiEvent {
  type: 'approval_requested'
  approval: UiApprovalRequest
}

export interface ApprovalResolvedUiEvent {
  type: 'approval_resolved'
  resolution: UiApprovalResolution
}

export interface ValidationUiEvent {
  type: 'validation'
  validation: UiValidation
}

export interface RunStoppedUiEvent {
  type: 'run_stopped'
  reason: StopReason
}

export interface ErrorUiEvent {
  type: 'error'
  code: string
  message: string
}

export type UiEventKind =
  | RunStartedUiEvent
  | TurnStartedUiEvent
  | MessageUiEvent
  | MessageDeltaUiEvent
  | ReasoningUiEvent
  | ToolCallUiEvent
  | FileChangeUiEvent
  | TodoListUiEvent
  | UsageUiEvent
  | ApprovalRequestedUiEvent
  | ApprovalResolvedUiEvent
  | ValidationUiEvent
  | RunStoppedUiEvent
  | ErrorUiEvent

export type UiEventType = UiEventKind['type']

export const UI_EVENT_TYPES = [
  'run_started',
  'turn_started',
  'message',
  'message_delta',
  'reasoning',
  'tool_call',
  'file_change',
  'todo_list',
  'usage',
  'approval_requested',
  'approval_resolved',
  'validation',
  'run_stopped',
  'error',
] as const satisfies readonly UiEventType[]

export interface UiEventEnvelope {
  schema_version: typeof UI_SCHEMA_VERSION
  event_id: EventId
  run_id: RunId
  turn_id?: TurnId
  call_id?: CallId
  sequence: number
  occurred_at: string
  kind: UiEventKind
}
