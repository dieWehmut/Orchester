import type {
  RunReplayRequestDto,
  RunReplayResponseDto,
  RunSnapshotDto,
  RunSummaryDto,
  StartRunRequest,
  StartRunResponse,
} from '@orchester/protokoll'

import type { HttpClient, HttpRequestOptions } from './http'

export interface RunRequestOptions {
  signal?: AbortSignal
}

export interface StartRunOptions extends RunRequestOptions {
  /** Sent as a header so retries do not duplicate a user submission. */
  idempotencyKey?: string
}

/** A resume request keeps the shared start-run wire fields and requires a handle. */
export type ResumeRunRequest = StartRunRequest & { resume: string }

export interface RunsApi {
  start: (request: StartRunRequest, options?: StartRunOptions) => Promise<StartRunResponse>
  resume: (request: ResumeRunRequest, options?: StartRunOptions) => Promise<StartRunResponse>
  snapshot: (runId: string, options?: RunRequestOptions) => Promise<RunSnapshotDto>
  replay: (
    runId: string,
    request: RunReplayRequestDto,
    options?: RunRequestOptions,
  ) => Promise<RunReplayResponseDto>
  cancel: (runId: string, options?: RunRequestOptions) => Promise<RunSummaryDto>
}

function requestOptions(
  options: StartRunOptions | RunRequestOptions | undefined,
): HttpRequestOptions | undefined {
  if (!options) return undefined
  const hasIdempotencyKey = 'idempotencyKey' in options && Boolean(options.idempotencyKey)
  if (!options.signal && !hasIdempotencyKey) return undefined

  const result: HttpRequestOptions = {}
  if (options.signal) result.signal = options.signal
  if (hasIdempotencyKey && 'idempotencyKey' in options) {
    result.headers = { 'Idempotency-Key': options.idempotencyKey }
  }
  return result
}

function runPath(runId: string): string {
  return `/runs/${encodeURIComponent(runId)}`
}

export function createRunsApi(http: HttpClient): RunsApi {
  const start = (request: StartRunRequest, options?: StartRunOptions) => {
    const init = requestOptions(options)
    return init
      ? http.post<StartRunResponse>('/runs', request, init)
      : http.post<StartRunResponse>('/runs', request)
  }

  return {
    start,
    resume: (request, options) => start(request, options),
    snapshot: (runId, options) => {
      const init = requestOptions(options)
      const path = runPath(runId)
      return init
        ? http.get<RunSnapshotDto>(path, init)
        : http.get<RunSnapshotDto>(path)
    },
    replay: (runId, request, options) => {
      const init = requestOptions(options)
      const path = `${runPath(runId)}/replay`
      return init
        ? http.post<RunReplayResponseDto>(path, request, init)
        : http.post<RunReplayResponseDto>(path, request)
    },
    cancel: (runId, options) => {
      const init = requestOptions(options)
      const path = `${runPath(runId)}/cancel`
      return init
        ? http.post<RunSummaryDto>(path, undefined, init)
        : http.post<RunSummaryDto>(path)
    },
  }
}
