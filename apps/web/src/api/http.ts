export interface HttpRequestOptions extends Omit<RequestInit, 'body' | 'headers' | 'method'> {
  headers?: HeadersInit
  method?: string
}

export interface HttpClientOptions {
  baseUrl?: string
  fetch?: typeof globalThis.fetch
  requestId?: () => string
  csrfToken?: () => string | null | undefined
}

export class HttpResponseError extends Error {
  readonly response: Response
  readonly payload: unknown

  constructor(response: Response, payload: unknown) {
    super(`HTTP request failed with status ${response.status}`)
    this.name = 'HttpResponseError'
    this.response = response
    this.payload = payload
  }
}

export interface HttpClient {
  request<T>(path: string, options?: HttpRequestOptions, body?: unknown): Promise<T>
  get<T>(path: string, options?: HttpRequestOptions): Promise<T>
  post<T>(path: string, body?: unknown, options?: HttpRequestOptions): Promise<T>
  put<T>(path: string, body?: unknown, options?: HttpRequestOptions): Promise<T>
  patch<T>(path: string, body?: unknown, options?: HttpRequestOptions): Promise<T>
  delete<T>(path: string, options?: HttpRequestOptions): Promise<T>
}

const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS'])

function defaultRequestId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID()
  return `request-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}

function joinUrl(baseUrl: string, path: string): string {
  if (/^https?:\/\//.test(path)) return path
  const normalizedBase = baseUrl.replace(/\/$/, '')
  const normalizedPath = path.startsWith('/') ? path : `/${path}`
  return `${normalizedBase}${normalizedPath}`
}

async function readPayload(response: Response): Promise<unknown> {
  if (response.status === 204) return undefined
  const contentType = response.headers.get('content-type') ?? ''
  if (contentType.includes('application/json')) {
    try {
      return await response.json()
    } catch {
      return undefined
    }
  }
  return response.text()
}

export function createHttpClient(options: HttpClientOptions = {}): HttpClient {
  const baseUrl = options.baseUrl ?? '/api/v1'
  const request = options.fetch ?? globalThis.fetch.bind(globalThis)
  const requestId = options.requestId ?? defaultRequestId
  const csrfToken = options.csrfToken ?? (() => undefined)

  async function send<T>(path: string, init: HttpRequestOptions = {}, body?: unknown): Promise<T> {
    const method = (init.method ?? (body === undefined ? 'GET' : 'POST')).toUpperCase()
    const headers = new Headers(init.headers)
    headers.set('accept', 'application/json')
    headers.set('x-request-id', requestId())
    if (body !== undefined) {
      headers.set('content-type', 'application/json')
    }
    if (!SAFE_METHODS.has(method)) {
      const token = csrfToken()
      if (token) headers.set('x-csrf-token', token)
    }

    const requestInit: RequestInit = {
      ...init,
      method,
      credentials: init.credentials ?? 'same-origin',
      headers,
    }
    if (body !== undefined) requestInit.body = JSON.stringify(body)

    const response = await request(joinUrl(baseUrl, path), requestInit)
    const payload = await readPayload(response)
    if (!response.ok) throw new HttpResponseError(response, payload)
    return payload as T
  }

  return {
    request: send,
    get: (path, init) => send(path, { ...init, method: 'GET' }),
    post: (path, body, init) => send(path, { ...init, method: 'POST' }, body),
    put: (path, body, init) => send(path, { ...init, method: 'PUT' }, body),
    patch: (path, body, init) => send(path, { ...init, method: 'PATCH' }, body),
    delete: (path, init) => send(path, { ...init, method: 'DELETE' }),
  }
}

export const apiClient = createHttpClient()
