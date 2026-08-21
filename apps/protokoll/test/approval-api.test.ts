import { describe, expect, it } from 'vitest'

import {
  approvalId,
  runId,
  type ApprovalDecisionRequestDto,
  type ApprovalDecisionResponseDto,
  type ApprovalQueueDto,
} from '../src/index'

const queue: ApprovalQueueDto = {
  run_id: runId('run-approval'),
  items: [
    {
      approval_id: approvalId('approval-1'),
      run_id: runId('run-approval'),
      row_version: 4,
      risk: 'workspace_write',
      action: 'write_file path=src/main.rs',
      reason: 'The requested change modifies source code',
      state: 'pending',
      created_at: '2026-08-19T00:00:00Z',
      expires_at: '2026-08-19T00:15:00Z',
    },
  ],
}

describe('approval API DTOs', () => {
  it('keeps queue items bound to their run and row version', () => {
    expect(queue.items[0]?.run_id).toBe(queue.run_id)
    expect(queue.items[0]?.row_version).toBe(4)
  })

  it('requires an idempotency key for a decision write', () => {
    const request: ApprovalDecisionRequestDto = {
      approval_id: approvalId('approval-1'),
      row_version: 4,
      decision: 'approved',
      idempotency_key: 'approval-1:4:approve:client-1',
    }
    const response: ApprovalDecisionResponseDto = {
      status: 'applied',
      approval_id: request.approval_id,
      row_version: 5,
      decision: request.decision,
    }

    expect(response.status).toBe('applied')
    expect(response.row_version).toBeGreaterThan(request.row_version)
  })

  it('represents stale and expired decisions without pretending success', () => {
    const stale: ApprovalDecisionResponseDto = {
      status: 'stale',
      approval_id: approvalId('approval-1'),
      row_version: 4,
      decision: 'stale',
    }
    const expired: ApprovalDecisionResponseDto = {
      status: 'expired',
      approval_id: approvalId('approval-2'),
      row_version: 2,
      decision: 'expired',
    }

    expect([stale.status, expired.status]).toEqual(['stale', 'expired'])
  })
})
