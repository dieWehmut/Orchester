import type { UiApprovalRequest, UiEventEnvelope } from '@orchester/protokoll'
import { timelineItemKey } from './event-key'
import {
  createEmptyRunView,
  type ApprovalView,
  type RunView,
  type TimelineItem,
  type ToolInvocationView,
  type TurnView,
} from './run-view'

export function approvalView(request: UiApprovalRequest, sequence: number | null): ApprovalView {
  return {
    key: `approval:${request.approval_id}`,
    approvalId: request.approval_id,
    runId: request.run_id,
    rowVersion: request.row_version,
    risk: request.risk,
    action: request.action,
    reason: request.reason,
    expiresAt: request.expires_at ?? null,
    state: 'pending',
    requestedSequence: sequence,
    resolvedSequence: null,
  }
}

/** Projects only the contiguous, deduplicated journal supplied by project-run. */
export function projectConversation(events: readonly UiEventEnvelope[]): RunView {
  const empty = createEmptyRunView(events[0]?.run_id ?? null)
  const timeline: TimelineItem[] = []
  const itemIndexes = new Map<string, number>()
  const tools = new Map<string, ToolInvocationView>()
  const approvals = new Map<string, ApprovalView>()
  const turns = new Map<string, TurnView>()
  const streams = new Map<string, number>()
  let usage = empty.usage
  let validation = empty.validation
  let todos = empty.todos

  function put(item: TimelineItem): void {
    const index = itemIndexes.get(item.key)
    if (index === undefined) {
      itemIndexes.set(item.key, timeline.length)
      timeline.push(item)
    } else {
      const first = timeline[index]!
      timeline[index] = { ...item, sequence: first.sequence, occurredAt: first.occurredAt, turnId: first.turnId } as TimelineItem
    }
  }

  for (const event of events) {
    const kind = event.kind
    const base = {
      key: timelineItemKey(event),
      sequence: event.sequence,
      occurredAt: event.occurred_at,
      turnId: event.turn_id ?? null,
    }
    const turnKey = `turn:${event.turn_id ?? 'unassigned'}`
    if (kind.type !== 'run_started' && kind.type !== 'run_stopped' && !turns.has(turnKey)) {
      turns.set(turnKey, { key: turnKey, id: base.turnId, startedAt: event.occurred_at, endedAt: null, items: [] })
    }

    switch (kind.type) {
      case 'message':
        put({ ...base, type: 'message', role: 'assistant', text: kind.text, final: true })
        streams.delete(turnKey)
        break
      case 'message_delta': {
        const index = streams.get(turnKey)
        const previous = index === undefined ? undefined : timeline[index]
        if (previous?.type === 'message') {
          timeline[index!] = { ...previous, text: previous.text + kind.text, final: kind.final }
        } else {
          streams.set(turnKey, timeline.length)
          put({ ...base, key: `${base.key}:${event.sequence}`, type: 'message', role: 'assistant', text: kind.text, final: kind.final })
        }
        if (kind.final) streams.delete(turnKey)
        break
      }
      case 'reasoning':
        put({ ...base, type: 'reasoning', text: kind.text })
        break
      case 'tool_call': {
        const item = { ...base, type: 'tool' as const, callId: kind.call_id, name: kind.name, state: kind.state, detail: kind.detail ?? null }
        const previous = tools.get(kind.call_id)
        tools.set(kind.call_id, {
          key: base.key, callId: kind.call_id, name: kind.name, state: kind.state,
          detail: kind.detail ?? previous?.detail ?? null,
          firstSequence: previous?.firstSequence ?? event.sequence,
          lastSequence: event.sequence, history: [...(previous?.history ?? []), item],
        })
        put({ ...item, detail: tools.get(kind.call_id)!.detail })
        break
      }
      case 'file_change':
        put({ ...base, type: 'file_change', path: kind.path, kind: kind.kind })
        break
      case 'todo_list':
        todos = kind.items.map((item) => ({ ...item }))
        put({ ...base, type: 'todo_list', items: todos })
        break
      case 'usage':
        usage = {
          input_tokens: kind.input_tokens, output_tokens: kind.output_tokens,
          cached_input_tokens: kind.cached_input_tokens, reasoning_output_tokens: kind.reasoning_output_tokens,
        }
        break
      case 'validation':
        validation = { ...kind.validation }
        put({ ...base, type: 'validation', validation })
        break
      case 'approval_requested': {
        const request = kind.approval
        if (request.run_id !== event.run_id) throw new RangeError('approval belongs to another run')
        const previous = approvals.get(request.approval_id)
        if (previous && previous.rowVersion >= request.row_version) break
        approvals.set(request.approval_id, approvalView(request, event.sequence))
        put({ ...base, type: 'approval', approvalId: request.approval_id, state: 'pending', request: { ...request }, decision: null })
        break
      }
      case 'approval_resolved': {
        const resolution = kind.resolution
        const previous = approvals.get(resolution.approval_id)
        if (previous && previous.rowVersion > resolution.row_version) break
        const index = itemIndexes.get(base.key)
        const item = index === undefined ? undefined : timeline[index]
        approvals.set(resolution.approval_id, {
          ...(previous ?? {
            key: base.key, approvalId: resolution.approval_id, runId: event.run_id,
            risk: '', action: '', reason: '', expiresAt: null, requestedSequence: null,
          }),
          rowVersion: resolution.row_version, state: resolution.decision, resolvedSequence: event.sequence,
        })
        put({ ...base, type: 'approval', approvalId: resolution.approval_id, state: resolution.decision,
          request: item?.type === 'approval' ? item.request : null, decision: resolution.decision })
        break
      }
      case 'error':
        put({ ...base, type: 'error', code: kind.code, message: kind.message })
        break
      case 'turn_started':
        streams.delete(turnKey)
        break
      case 'run_stopped':
        for (const [key, turn] of turns) {
          if (turn.endedAt === null) turns.set(key, { ...turn, endedAt: event.occurred_at })
        }
        break
      case 'run_started':
        break
    }
  }

  return {
    ...empty, timeline,
    turns: [...turns.values()].map((turn) => ({ ...turn, items: timeline.filter((item) => item.turnId === turn.id) })),
    tools: [...tools.values()], approvals: [...approvals.values()], usage, validation, todos,
    fileChanges: timeline.filter((item) => item.type === 'file_change'),
    errors: timeline.filter((item) => item.type === 'error'),
  }
}
