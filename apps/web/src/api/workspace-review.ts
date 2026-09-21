import type { WorkspaceReviewDto } from '@orchester/protokoll'

import type { HttpClient } from './http'

export interface WorkspaceReviewRequestOptions {
  signal?: AbortSignal
}

export interface WorkspaceReviewApi {
  read: (options?: WorkspaceReviewRequestOptions) => Promise<WorkspaceReviewDto>
}

/**
 * The Review tab's data source.
 *
 * The runtime reads the working copy, not the browser: a page cannot see the
 * index, and section 0 of the design spec makes the runtime the source of truth
 * for exactly that reason.
 */
export function createWorkspaceReviewApi(http: HttpClient): WorkspaceReviewApi {
  return {
    read: ({ signal } = {}) =>
      signal
        ? http.get<WorkspaceReviewDto>('/workspace/review', { signal })
        : http.get<WorkspaceReviewDto>('/workspace/review'),
  }
}
