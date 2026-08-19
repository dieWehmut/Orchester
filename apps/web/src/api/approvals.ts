import type {
  ApprovalDecisionRequestDto,
  ApprovalDecisionResponseDto,
  ApprovalQueueDto,
} from '@orchester/protokoll'

import type { HttpClient, HttpRequestOptions } from './http'

export interface ApprovalRequestOptions {
  signal?: AbortSignal
}

export interface ApprovalsApi {
  list: (runId: string, options?: ApprovalRequestOptions) => Promise<ApprovalQueueDto>
  decide: (
    runId: string,
    request: ApprovalDecisionRequestDto,
    options?: ApprovalRequestOptions,
  ) => Promise<ApprovalDecisionResponseDto>
}

function requestOptions(options?: ApprovalRequestOptions): HttpRequestOptions | undefined {
  return options?.signal ? { signal: options.signal } : undefined
}

function runPath(runId: string): string {
  return `/runs/${encodeURIComponent(runId)}/approvals`
}

export function createApprovalsApi(http: HttpClient): ApprovalsApi {
  return {
    list: (runId, options) => {
      const init = requestOptions(options)
      const path = runPath(runId)
      return init
        ? http.get<ApprovalQueueDto>(path, init)
        : http.get<ApprovalQueueDto>(path)
    },
    decide: (runId, request, options) => {
      const init = requestOptions(options)
      const path = `${runPath(runId)}/${encodeURIComponent(request.approval_id)}`
      return init
        ? http.post<ApprovalDecisionResponseDto>(path, request, init)
        : http.post<ApprovalDecisionResponseDto>(path, request)
    },
  }
}
