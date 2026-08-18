import type { RunId, TurnId, CallId, UiToolState } from './ui'

/** A tool invocation is identified by call_id, never by its display name. */
export interface ToolInvocationDto {
  call_id: CallId
  run_id: RunId
  turn_id?: TurnId
  name: string
  state: UiToolState
  detail?: string
  started_at?: string
}

export interface ToolInvocationResultDto {
  call_id: CallId
  state: Exclude<UiToolState, 'queued' | 'running'>
  detail?: string
  completed_at: string
}

export function toolInvocationKey(invocation: Pick<ToolInvocationDto, 'call_id'>): CallId {
  return invocation.call_id
}
