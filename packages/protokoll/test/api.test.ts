import { describe, expect, it } from 'vitest'

import {
  UI_SCHEMA_VERSION,
  approvalId,
  eventId,
  runId,
  type ApiErrorDto,
  type ResyncRequiredDto,
  type RunReplayRequestDto,
  type RunReplayResponseDto,
  type RunSnapshotDto,
} from '../src/index'

const snapshot: RunSnapshotDto = {
  run_id: runId('run-1'),
  state: 'awaiting_approval',
  events: [
    {
      schema_version: UI_SCHEMA_VERSION,
      event_id: eventId('event-1'),
      run_id: runId('run-1'),
      sequence: 1,
      occurred_at: '2026-08-19T00:00:00Z',
      kind: {
        type: 'approval_requested',
        approval: {
          approval_id: approvalId('approval-1'),
          run_id: runId('run-1'),
          row_version: 1,
          risk: 'high',
          action: 'write_file path=src/main.rs',
          reason: 'workspace write',
        },
      },
    },
  ],
  pending_approvals: [],
  oldest_sequence: 1,
  latest_sequence: 1,
  next_sequence: 2,
  updated_at: '2026-08-19T00:00:01Z',
}

describe('run replay DTOs', () => {
  it('models a bounded snapshot and replay response', () => {
    const request: RunReplayRequestDto = { after_sequence: 0, limit: 100 }
    const response: RunReplayResponseDto = {
      run_id: runId('run-1'),
      events: snapshot.events,
      first_sequence: 1,
      last_sequence: 1,
      has_more: false,
    }

    expect(request.after_sequence).toBe(0)
    expect(response.events).toHaveLength(1)
    expect(response.last_sequence).toBe(1)
    expect(snapshot.next_sequence).toBe(response.last_sequence! + 1)
  })

  it('models an explicit resync response when replay is unavailable', () => {
    const response: ResyncRequiredDto = {
      type: 'resync_required',
      run_id: runId('run-1'),
      requested_after_sequence: 0,
      oldest_sequence: 10,
      latest_sequence: 25,
      reason: 'retention_exceeded',
    }

    expect(response.type).toBe('resync_required')
  })

  it('keeps API errors machine-readable and request-correlated', () => {
    const error: ApiErrorDto = {
      error: 'Replay is no longer retained',
      code: 'resync_required',
      request_id: 'req-1',
      retryable: false,
    }
    expect(error.code).toBe('resync_required')
  })
})
