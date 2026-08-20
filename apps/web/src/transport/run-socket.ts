import {
  parseRunStreamFrameJson,
  type ResyncRequiredDto,
  type UiEventEnvelope,
} from '@orchester/protokoll'

import {
  createReconnectBackoff,
  type ReconnectBackoffOptions,
} from './backoff'

export type RunSocketStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'offline'
  | 'closed'
  | 'fatal'

export interface WebSocketLike {
  readonly readyState: number
  onopen: ((event: Event) => void) | null
  onmessage: ((event: MessageEvent) => void) | null
  onclose: ((event: CloseEvent) => void) | null
  onerror: ((event: Event) => void) | null
  close: (code?: number, reason?: string) => void
}

export interface RunSocketOptions {
  ticketProvider: () => string | Promise<string>
  afterSequence?: () => number
  webSocketFactory?: (url: string) => WebSocketLike
  backoff?: ReconnectBackoffOptions
  schedule?: (callback: () => void, delay: number) => unknown
  cancelScheduled?: (handle: unknown) => void
  onEvent?: (event: UiEventEnvelope) => void
  onResyncRequired?: (frame: ResyncRequiredDto) => void
  onError?: (error: Error) => void
  onStatus?: (status: RunSocketStatus) => void
}

export interface RunSocket {
  readonly status: RunSocketStatus
  connect: () => Promise<void>
  close: () => void
}

function defaultWebSocketFactory(url: string): WebSocketLike {
  return new WebSocket(url)
}

function defaultSchedule(callback: () => void, delay: number): unknown {
  return globalThis.setTimeout(callback, delay)
}

function defaultCancelScheduled(handle: unknown): void {
  globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>)
}

function withReplayCursor(ticketUrl: string, afterSequence: number): string {
  const url = new URL(ticketUrl)
  if (url.protocol !== 'ws:' && url.protocol !== 'wss:') {
    throw new TypeError('Run event URL must use ws or wss')
  }
  if (!Number.isSafeInteger(afterSequence) || afterSequence < 0) {
    throw new RangeError('Run event replay cursor must be a non-negative integer')
  }
  url.searchParams.set('after_sequence', String(afterSequence))
  return url.toString()
}

function asError(cause: unknown): Error {
  return cause instanceof Error ? cause : new Error(String(cause))
}

export function createRunSocket(options: RunSocketOptions): RunSocket {
  const createWebSocket = options.webSocketFactory ?? defaultWebSocketFactory
  const backoff = createReconnectBackoff(options.backoff)
  const schedule = options.schedule ?? defaultSchedule
  const cancelScheduled = options.cancelScheduled ?? defaultCancelScheduled
  let socket: WebSocketLike | null = null
  let currentStatus: RunSocketStatus = 'idle'
  let manuallyClosed = false
  let connectionPromise: Promise<void> | null = null
  let reconnectHandle: unknown | null = null

  function setStatus(status: RunSocketStatus): void {
    if (currentStatus === status) return
    currentStatus = status
    options.onStatus?.(status)
  }

  function reportError(cause: unknown): Error {
    const error = asError(cause)
    options.onError?.(error)
    return error
  }

  function scheduleReconnect(cause?: unknown): void {
    if (manuallyClosed || reconnectHandle !== null) return
    const delay = backoff.next()
    if (delay === null) {
      setStatus('fatal')
      const error = new Error('Run socket reconnect budget exhausted')
      if (cause !== undefined) error.cause = cause
      reportError(error)
      return
    }

    setStatus('reconnecting')
    reconnectHandle = schedule(() => {
      reconnectHandle = null
      const reconnecting = openConnection(true)
      void reconnecting.catch(() => undefined)
    }, delay)
  }

  function openConnection(reconnecting: boolean): Promise<void> {
    setStatus(reconnecting ? 'reconnecting' : 'connecting')
    let ticket: string | Promise<string>
    try {
      ticket = options.ticketProvider()
    } catch (cause) {
      const error = reportError(cause)
      if (reconnecting) scheduleReconnect(error)
      else setStatus('fatal')
      return Promise.reject(error)
    }
    const pending = Promise.resolve(ticket).then(
      (ticketUrl) =>
        new Promise<void>((resolve, reject) => {
          let opened = false
          if (manuallyClosed) {
            reject(new Error('Run socket is closed'))
            return
          }
          try {
            socket = createWebSocket(
              withReplayCursor(ticketUrl, options.afterSequence?.() ?? 0),
            )
          } catch (cause) {
            setStatus('fatal')
            reject(reportError(cause))
            return
          }

          socket.onopen = () => {
            opened = true
            backoff.reset()
            setStatus('connected')
            resolve()
          }
          socket.onmessage = (message) => {
            if (typeof message.data !== 'string') {
              reportError(new TypeError('Invalid run stream frame: expected text'))
              return
            }
            const frame = parseRunStreamFrameJson(message.data)
            if (!frame) {
              reportError(new TypeError('Invalid run stream frame'))
              return
            }
            if (frame.type === 'event') {
              options.onEvent?.(frame.event)
              if (frame.event.kind.type === 'run_stopped') close()
            } else {
              options.onResyncRequired?.(frame)
            }
          }
          socket.onerror = () => {
            const error = reportError(new Error('Run socket connection failed'))
            if (!opened) reject(error)
          }
          socket.onclose = () => {
            socket = null
            connectionPromise = null
            if (manuallyClosed) {
              setStatus('closed')
              return
            }
            let closeError: Error | undefined
            if (!opened) {
              closeError = reportError(new Error('Run socket closed before opening'))
              reject(closeError)
            }
            scheduleReconnect(closeError)
          }
        }),
      (cause) => {
        const error = reportError(cause)
        if (reconnecting) scheduleReconnect(error)
        else setStatus('fatal')
        throw error
      },
    )
    connectionPromise = pending
    return pending
  }

  function connect(): Promise<void> {
    if (connectionPromise) return connectionPromise
    if (manuallyClosed) return Promise.reject(new Error('Run socket is closed'))

    connectionPromise = openConnection(false)
    return connectionPromise
  }

  function close(): void {
    manuallyClosed = true
    if (reconnectHandle !== null) {
      cancelScheduled(reconnectHandle)
      reconnectHandle = null
    }
    const activeSocket = socket
    socket = null
    connectionPromise = null
    setStatus('closed')
    activeSocket?.close(1000, 'client closed')
  }

  return {
    get status() {
      return currentStatus
    },
    connect,
    close,
  }
}
