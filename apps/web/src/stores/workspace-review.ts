import {
  parseWorkspaceReview,
  type WorkspaceReviewChangeDto,
  type WorkspaceReviewDto,
} from '@orchester/protokoll'
import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'

import type { ApiError } from '../api/errors'
import { normalizeApiError } from '../api/errors'
import type { WorkspaceReviewApi } from '../api/workspace-review'

export type WorkspaceReviewStatus = 'idle' | 'loading' | 'refreshing' | 'ready' | 'stale' | 'error'

/**
 * The Review tab's data, task U5-01 of the implementation plan.
 *
 * The store holds the two halves the filters need and joins them once. The
 * working copy change set comes from the runtime; the turn a change arrived in
 * comes from the run's own projection, because which turn work belongs to is a
 * fact about the event stream rather than about git. Joining here keeps the
 * component from re-deriving a turn on every render.
 */
export const useWorkspaceReviewStore = defineStore('workspaceReview', () => {
  const status = ref<WorkspaceReviewStatus>('idle')
  const review = shallowRef<WorkspaceReviewDto | null>(null)
  const error = shallowRef<ApiError | null>(null)

  const changes = computed<readonly WorkspaceReviewChangeDto[]>(
    () => review.value?.changes ?? [],
  )
  const branchChanges = computed<readonly string[]>(() => review.value?.branch_changes ?? [])
  const branch = computed<string | null>(() => review.value?.branch ?? null)

  let api: WorkspaceReviewApi | null = null
  let generation = 0

  function configure(nextApi: WorkspaceReviewApi): void {
    api = nextApi
  }

  async function load(): Promise<void> {
    const currentApi = api
    const currentGeneration = ++generation
    if (!currentApi) {
      error.value = normalizeApiError(new TypeError('workspace review API unavailable'))
      status.value = review.value ? 'stale' : 'error'
      return
    }

    status.value = review.value ? 'refreshing' : 'loading'
    error.value = null
    try {
      const payload = await currentApi.read()
      if (currentGeneration !== generation) return
      // The payload decides whether it is the contract the tab can render. A
      // shape this build does not understand is a stale answer, not a reason
      // to draw paths the guard could not vouch for.
      const parsed = parseWorkspaceReview(payload)
      if (parsed === null) {
        error.value = normalizeApiError(new TypeError('workspace review payload rejected'))
        status.value = review.value ? 'stale' : 'error'
        return
      }
      review.value = parsed
      status.value = 'ready'
    } catch (cause) {
      if (currentGeneration !== generation) return
      error.value = normalizeApiError(cause)
      status.value = review.value ? 'stale' : 'error'
    }
  }

  function reset(): void {
    generation += 1
    status.value = 'idle'
    review.value = null
    error.value = null
  }

  return {
    status,
    review,
    changes,
    branchChanges,
    branch,
    error,
    configure,
    load,
    reset,
  }
})
