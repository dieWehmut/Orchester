import {
  parseAgentFleetStreamFrameJson,
  type AgentFleetSnapshotDto,
  type AgentFleetStreamFrameDto,
} from '@orchester/protokoll'

import type { WebSocketLike } from './run-socket'
import {
  createReconnectBackoff,
  type ReconnectBackoffOptions,
} from './backoff'

export type AgentStatusSocketStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'closed'
  | 'fatal'

export type AgentStatusHeartbeat = Extract<
  AgentFleetStreamFrameDto,
  { type: 'heartbeat' }
>

export interface AgentStatusSocketOptions {
  urlProvider?: () => string | Promise<string>
  webSocketFactory?: (url: string) => WebSocketLike
  backoff?: ReconnectBackoffOptions
  schedule?: (callback: () => void, delay: number) => unknown
  cancelScheduled?: (handle: unknown) => void
  onSnapshot?: (snapshot: AgentFleetSnapshotDto) => void
  onHeartbeat?: (heartbeat: AgentStatusHeartbeat) => void
  onError?: (error: Error) => void
  onStatus?: (status: AgentStatusSocketStatus) => void
}

export interface AgentStatusSocket {
  readonly status: AgentStatusSocketStatus
  connect: () => Promise<void>
  close: () => void
}

function defaultWebSocketFactory(url: string): WebSocketLike {
  return new WebSocket(url)
}

function defaultUrlProvider(): string {
  const url = new URL('/api/v1/agents/status/ws', globalThis.location.href)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  return url.toString()
}

function defaultSchedule(callback: () => void, delay: number): unknown {
  return globalThis.setTimeout(callback, delay)
}

function defaultCancelScheduled(handle: unknown): void {
  globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>)
}

function validateSocketUrl(raw: string): string {
  const url = new URL(raw)
  if (url.protocol !== 'ws:' && url.protocol !== 'wss:') {
    throw new TypeError('Agent status URL must use ws or wss')
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new TypeError('Agent status URL must not contain credentials or tokens')
  }
  return url.toString()
}

function asError(cause: unknown): Error {
  return cause instanceof Error ? cause : new Error(String(cause))
}

export function createAgentStatusSocket(
  options: AgentStatusSocketOptions = {},
): AgentStatusSocket {
  const urlProvider = options.urlProvider ?? defaultUrlProvider
  const createWebSocket = options.webSocketFactory ?? defaultWebSocketFactory
  const backoff = createReconnectBackoff(options.backoff)
  const schedule = options.schedule ?? defaultSchedule
  const cancelScheduled = options.cancelScheduled ?? defaultCancelScheduled
  let socket: WebSocketLike | null = null
  let currentStatus: AgentStatusSocketStatus = 'idle'
  let manuallyClosed = false
  let connectionPromise: Promise<void> | null = null
  let reconnectHandle: unknown | null = null

  function setStatus(status: AgentStatusSocketStatus): void {
    if (status === currentStatus) return
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
      const error = new Error('Agent status socket reconnect budget exhausted')
      if (cause !== undefined) error.cause = cause
      reportError(error)
      return
    }
    setStatus('reconnecting')
    reconnectHandle = schedule(() => {
      reconnectHandle = null
      void openConnection(true).catch(() => undefined)
    }, delay)
  }

  function openConnection(reconnecting: boolean): Promise<void> {
    setStatus(reconnecting ? 'reconnecting' : 'connecting')
    let rawUrl: string | Promise<string>
    try {
      rawUrl = urlProvider()
    } catch (cause) {
      const error = reportError(cause)
      if (reconnecting) scheduleReconnect(error)
      else setStatus('fatal')
      return Promise.reject(error)
    }

    const pending = Promise.resolve(rawUrl).then(
      (providedUrl) =>
        new Promise<void>((resolve, reject) => {
          let opened = false
          if (manuallyClosed) {
            reject(new Error('Agent status socket is closed'))
            return
          }
          try {
            socket = createWebSocket(validateSocketUrl(providedUrl))
          } catch (cause) {
            const error = reportError(cause)
            setStatus('fatal')
            reject(error)
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
              reportError(new TypeError('Invalid agent status stream frame: expected text'))
              return
            }
            const frame = parseAgentFleetStreamFrameJson(message.data)
            if (frame === null) {
              reportError(new TypeError('Invalid agent status stream frame'))
              return
            }
            if (frame.type === 'snapshot') options.onSnapshot?.(frame.snapshot)
            else options.onHeartbeat?.(frame)
          }
          socket.onerror = () => {
            const error = reportError(new Error('Agent status socket connection failed'))
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
              closeError = reportError(new Error('Agent status socket closed before opening'))
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
    if (connectionPromise !== null) return connectionPromise
    if (manuallyClosed) return Promise.reject(new Error('Agent status socket is closed'))
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
