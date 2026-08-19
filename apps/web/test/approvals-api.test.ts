import type {
  ApprovalDecisionRequestDto,
  ApprovalDecisionResponseDto,
  ApprovalQueueDto,
} from '@orchester/protokoll'
import { approvalId, runId } from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createApprovalsApi } from '../src/api/approvals'
import type { HttpClient } from '../src/api/http'

describe('approval API client', () => {
  it('lists the redacted queue for an opaque run id', async () => {
    const queue: ApprovalQueueDto = { run_id: runId('run-1'), items: [] }
    const get = vi.fn(async () => queue)
    const api = createApprovalsApi({ get } as unknown as HttpClient)

    await expect(api.list('run/a')).resolves.toBe(queue)
    expect(get).toHaveBeenCalledWith('/runs/run%2Fa/approvals')
  })

  it('sends the protocol row version and idempotency key for a decision', async () => {
    const request: ApprovalDecisionRequestDto = {
      approval_id: approvalId('approval-1'),
      row_version: 4,
      decision: 'approved',
      idempotency_key: 'approval-1:4:client-1',
    }
    const response = {} as ApprovalDecisionResponseDto
    const post = vi.fn(async () => response)
    const api = createApprovalsApi({ post } as unknown as HttpClient)

    const controller = new AbortController()
    await expect(
      api.decide('run/a', request, { signal: controller.signal }),
    ).resolves.toBe(response)
    expect(post).toHaveBeenCalledWith(
      '/runs/run%2Fa/approvals/approval-1',
      request,
      { signal: controller.signal },
    )
  })
})
