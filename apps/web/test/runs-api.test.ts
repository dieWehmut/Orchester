import type {
  RunReplayResponseDto,
  RunSnapshotDto,
  RunSummaryDto,
  StartRunResponse,
} from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createRunsApi } from '../src/api/runs'
import type { HttpClient } from '../src/api/http'

describe('run API client', () => {
  it('starts a run with the protocol request and an idempotency header', async () => {
    const response: StartRunResponse = { run_id: 'run-1', events_url: '/events/run-1' }
    const post = vi.fn(async () => response)
    const api = createRunsApi({ post } as unknown as HttpClient)

    await expect(
      api.start({ prompt: 'inspect the workspace' }, { idempotencyKey: 'request-1' }),
    ).resolves.toBe(response)
    expect(post).toHaveBeenCalledWith(
      '/runs',
      { prompt: 'inspect the workspace' },
      { headers: { 'Idempotency-Key': 'request-1' } },
    )
  })

  it('loads a snapshot and replays after a sequence with an abort signal', async () => {
    const snapshot = {} as RunSnapshotDto
    const replay = {} as RunReplayResponseDto
    const get = vi.fn(async () => snapshot)
    const post = vi.fn(async () => replay)
    const controller = new AbortController()
    const api = createRunsApi({ get, post } as unknown as HttpClient)

    await expect(api.snapshot('run/a', { signal: controller.signal })).resolves.toBe(snapshot)
    await expect(
      api.replay('run/a', { after_sequence: 7, limit: 25 }, { signal: controller.signal }),
    ).resolves.toBe(replay)

    expect(get).toHaveBeenCalledWith('/runs/run%2Fa', { signal: controller.signal })
    expect(post).toHaveBeenCalledWith(
      '/runs/run%2Fa/replay',
      { after_sequence: 7, limit: 25 },
      { signal: controller.signal },
    )
  })

  it('cancels a run and resumes through the same protocol start request', async () => {
    const summary = {} as RunSummaryDto
    const response: StartRunResponse = { run_id: 'run-2', events_url: '/events/run-2' }
    const post = vi.fn()
      .mockResolvedValueOnce(summary)
      .mockResolvedValueOnce(response)
    const api = createRunsApi({ post } as unknown as HttpClient)

    await expect(api.cancel('run-1')).resolves.toBe(summary)
    await expect(
      api.resume({ prompt: 'continue the workspace task', resume: 'resume-handle' }),
    ).resolves.toBe(response)

    expect(post).toHaveBeenNthCalledWith(1, '/runs/run-1/cancel')
    expect(post).toHaveBeenNthCalledWith(2, '/runs', {
      prompt: 'continue the workspace task',
      resume: 'resume-handle',
    })
  })
})
