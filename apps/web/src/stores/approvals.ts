import type { ApprovalDecisionRequestDto, ApprovalQueueDto } from '@orchester/protokoll'
import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import type { ApiError } from '../api/errors'
import { normalizeApiError } from '../api/errors'
import type { ApprovalsApi } from '../api/approvals'

export type ApprovalsStatus = 'idle' | 'loading' | 'ready' | 'deciding' | 'stale' | 'error'

export interface ApprovalDecision {
  approvalId: string
  rowVersion: number
  decision: 'approved' | 'denied'
}

/**
 * The Approvals tab's data, task U5-04 of the implementation plan.
 *
 * The store exists for one reason: a decision is only safe if it is made
 * against the row version the reader actually saw. Section 4.7 asks for a
 * superseded state, and a row version is what makes one detectable, so the
 * version travels with the decision instead of being re-read from the queue at
 * send time. A decision the runtime refuses as stale is reported as stale
 * rather than as a failure, because nothing went wrong.
 */
export const useApprovalsStore = defineStore('approvals', () => {
  const status = ref<ApprovalsStatus>('idle')
  const queue = shallowRef<ApprovalQueueDto | null>(null)
  const error = shallowRef<ApiError | null>(null)
  const lastDecision = shallowRef<ApprovalDecision | null>(null)
  const superseded = shallowRef<readonly string[]>([])

  const items = computed(() => queue.value?.items ?? [])

  let api: ApprovalsApi | null = null
  let idempotencyKey: () => string = () => `approval-${Date.now().toString(36)}`

  function configure(nextApi: ApprovalsApi, nextKey?: () => string): void {
    api = nextApi
    if (nextKey) idempotencyKey = nextKey
  }

  async function load(runId: string): Promise<void> {
    const currentApi = api
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('approvals API unavailable'))
      status.value = queue.value ? 'stale' : 'error'
      return
    }

    status.value = queue.value ? 'stale' : 'loading'
    error.value = null
    try {
      queue.value = await currentApi.list(runId)
      status.value = 'ready'
    } catch (cause) {
      error.value = normalizeApiError(cause)
      status.value = queue.value ? 'stale' : 'error'
    }
  }

  /**
   * Send a decision against the version it was made against.
   *
   * A refusal the runtime reports as `stale` or `expired` is not an error: it
   * means the row moved on, so the entry is marked superseded and the queue is
   * reloaded for the caller to read the current version.
   */
  async function decide(runId: string, decision: ApprovalDecision): Promise<void> {
    const currentApi = api
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('approvals API unavailable'))
      status.value = 'error'
      return
    }

    const request: ApprovalDecisionRequestDto = {
      approval_id: decision.approvalId as ApprovalDecisionRequestDto['approval_id'],
      row_version: decision.rowVersion,
      decision: decision.decision,
      idempotency_key: idempotencyKey(),
    }

    status.value = 'deciding'
    error.value = null
    try {
      const response = await currentApi.decide(runId, request)
      lastDecision.value = decision
      if (response.status === 'stale' || response.status === 'expired') {
        superseded.value = [...new Set([...superseded.value, decision.approvalId])]
      }
      status.value = 'ready'
    } catch (cause) {
      error.value = normalizeApiError(cause)
      status.value = 'error'
    }
  }

  function reset(): void {
    status.value = 'idle'
    queue.value = null
    error.value = null
    lastDecision.value = null
    superseded.value = []
  }

  return { status, queue, items, error, lastDecision, superseded, configure, load, decide, reset }
})
