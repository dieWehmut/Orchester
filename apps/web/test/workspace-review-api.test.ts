import { WORKSPACE_REVIEW_SCHEMA_VERSION, type WorkspaceReviewDto } from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import type { HttpClient } from '../src/api/http'
import { createWorkspaceReviewApi } from '../src/api/workspace-review'

describe('workspace review API client', () => {
  it('reads the review from the loopback route', async () => {
    const review: WorkspaceReviewDto = {
      schema_version: WORKSPACE_REVIEW_SCHEMA_VERSION,
      branch: 'main',
      changes: [],
      branch_changes: [],
    }
    const get = vi.fn(async () => review)
    const api = createWorkspaceReviewApi({ get } as unknown as HttpClient)

    await expect(api.read()).resolves.toBe(review)
    expect(get).toHaveBeenCalledWith('/workspace/review')
  })

  it('forwards an abort signal so a closed tab stops reading', async () => {
    const get = vi.fn(async () => ({}) as WorkspaceReviewDto)
    const api = createWorkspaceReviewApi({ get } as unknown as HttpClient)
    const controller = new AbortController()

    await api.read({ signal: controller.signal })

    expect(get).toHaveBeenCalledWith('/workspace/review', { signal: controller.signal })
  })
})
