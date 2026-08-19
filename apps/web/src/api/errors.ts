import type { ApiErrorCode, ApiErrorDto } from '@orchester/protokoll'

import { HttpResponseError } from './http'

export type NormalizedApiErrorCode = ApiErrorCode | 'network' | 'aborted' | 'unknown'

export class ApiError extends Error {
  readonly code: NormalizedApiErrorCode
  readonly requestId: string | null
  readonly retryable: boolean
  readonly status: number | null

  constructor(
    message: string,
    options: {
      code: NormalizedApiErrorCode
      requestId?: string | null
      retryable: boolean
      status?: number | null
    },
  ) {
    super(message)
    this.name = 'ApiError'
    this.code = options.code
    this.requestId = options.requestId ?? null
    this.retryable = options.retryable
    this.status = options.status ?? null
  }
}

const RETRYABLE_STATUS = new Set([408, 425, 429, 500, 502, 503, 504])
const API_CODES = new Set<ApiErrorCode>([
  'bad_request',
  'method_not_allowed',
  'not_found',
  'unauthorized',
  'forbidden',
  'conflict',
  'resync_required',
  'validation_failed',
  'runtime_error',
  'unavailable',
  'internal',
])

function isApiErrorDto(value: unknown): value is ApiErrorDto {
  if (!value || typeof value !== 'object') return false
  const candidate = value as Partial<ApiErrorDto>
  return typeof candidate.error === 'string' && typeof candidate.code === 'string' && API_CODES.has(candidate.code)
}

function fallbackCode(status: number): ApiErrorCode {
  if (status === 400) return 'bad_request'
  if (status === 401) return 'unauthorized'
  if (status === 403) return 'forbidden'
  if (status === 404) return 'not_found'
  if (status === 409) return 'conflict'
  if (status === 429) return 'runtime_error'
  return status >= 500 ? 'internal' : 'runtime_error'
}

export function normalizeApiError(cause: unknown): ApiError {
  if (cause instanceof ApiError) return cause
  if (cause instanceof DOMException && cause.name === 'AbortError') {
    return new ApiError('Request cancelled', { code: 'aborted', retryable: false })
  }
  if (cause instanceof HttpResponseError) {
    const status = cause.response.status
    const payload = isApiErrorDto(cause.payload) ? cause.payload : undefined
    return new ApiError(payload?.error ?? `Request failed (${status})`, {
      code: payload?.code ?? fallbackCode(status),
      requestId: payload?.request_id ?? cause.response.headers.get('x-request-id'),
      retryable: payload?.retryable ?? RETRYABLE_STATUS.has(status),
      status,
    })
  }
  if (cause instanceof TypeError) {
    return new ApiError('Unable to reach the Orchester runtime', {
      code: 'network',
      retryable: true,
    })
  }
  return new ApiError('Unexpected runtime response', { code: 'unknown', retryable: false })
}
