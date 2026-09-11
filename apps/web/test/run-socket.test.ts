import { beforeEach, describe, expect, it, vi } from 'vitest'

import { eventId, runId, type UiEventEnvelope } from '@orchester/protokoll'
import { createRunSocket, type WebSocketLike } from '../src/transport/run-socket'

interface ScheduledTask {
  callback: () => void
  delay: number
}

class FakeWebSocket implements WebSocketLike {
  readonly url: string
  readyState = 0
  onopen: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onclose: ((event: CloseEvent) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  closeCalls = 0

  constructor(url: string) {
    this.url = url
  }

  open(): void {
    this.readyState = 1
    this.onopen?.(new Event('open'))
  }

  message(data: string): void {
    this.onmessage?.({ data } as MessageEvent)
  }

  remoteClose(code = 1006): void {
    this.readyState = 3
    this.onclose?.({ code, wasClean: code === 1000 } as CloseEvent)
  }

  close(): void {
    this.closeCalls += 1
    this.readyState = 3
    this.onclose?.({ code: 1000, wasClean: true } as CloseEvent)
  }
}

const envelope: UiEventEnvelope = {
  schema_version: 1,
  event_id: eventId('event-1'),
  run_id: runId('run-1'),
  sequence: 1,
  occurred_at: '2026-08-20T00:00:00.000Z',
  kind: { type: 'run_started', title: 'demo' },
}

describe('run socket lifecycle', () => {
  let sockets: FakeWebSocket[]
  let factory: (url: string) => WebSocketLike
  let scheduled: ScheduledTask[]
  let schedule: (callback: () => void, delay: number) => number

  beforeEach(() => {
    sockets = []
    scheduled = []
    factory = vi.fn((url: string) => {
      const socket = new FakeWebSocket(url)
      sockets.push(socket)
      return socket
    })
    schedule = vi.fn((callback: () => void, delay: number) => {
      scheduled.push({ callback, delay })
      return scheduled.length
    })
  })

  it('opens the server-issued ticket URL with the replay cursor and dispatches frames', async () => {
    const events: UiEventEnvelope[] = []
    const resyncs: unknown[] = []
    const errors: Error[] = []
    const statuses: string[] = []
    const client = createRunSocket({
      ticketProvider: async () => 'wss://127.0.0.1/events/ticket-secret?scope=run-1',
      afterSequence: () => 7,
      webSocketFactory: factory,
      onEvent: (event) => events.push(event),
      onResyncRequired: (frame) => resyncs.push(frame),
      onError: (error) => errors.push(error),
      onStatus: (status) => statuses.push(status),
    })

    const connected = client.connect()
    await Promise.resolve()
    expect(sockets[0]?.url).toBe(
      'wss://127.0.0.1/events/ticket-secret?scope=run-1&after_sequence=7',
    )

    sockets[0]!.open()
    await connected
    sockets[0]!.message(JSON.stringify({ type: 'event', event: envelope }))
    sockets[0]!.message(
      JSON.stringify({
        type: 'resync_required',
        run_id: runId('run-1'),
        requested_after_sequence: 7,
        oldest_sequence: 8,
        latest_sequence: 9,
        reason: 'sequence_gap',
      }),
    )

    expect(events).toEqual([envelope])
    expect(resyncs).toHaveLength(1)
    expect(errors).toEqual([])
    expect(statuses).toEqual(['connecting', 'connected'])
  })

  it('reports malformed frames without exposing a raw parser exception', async () => {
    const errors: Error[] = []
    const client = createRunSocket({
      ticketProvider: () => 'ws://127.0.0.1/events/ticket',
      webSocketFactory: factory,
      onError: (error) => errors.push(error),
    })

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected
    sockets[0]!.message('{not-json')

    expect(errors).toHaveLength(1)
    expect(errors[0]?.message).toContain('Invalid run stream frame')
  })

  it('actively closes the socket and remains closed', async () => {
    const client = createRunSocket({
      ticketProvider: () => 'ws://127.0.0.1/events/ticket',
      webSocketFactory: factory,
    })

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected
    client.close()

    expect(sockets[0]!.closeCalls).toBe(1)
    expect(client.status).toBe('closed')
  })

  it('does not create a socket when closed while the ticket is pending', async () => {
    let resolveTicket!: (url: string) => void
    const ticket = new Promise<string>((resolve) => {
      resolveTicket = resolve
    })
    const client = createRunSocket({
      ticketProvider: () => ticket,
      webSocketFactory: factory,
    })

    const connected = client.connect()
    client.close()
    resolveTicket('ws://127.0.0.1/events/ticket')

    await expect(connected).rejects.toThrow('Run socket is closed')
    expect(sockets).toEqual([])
    expect(client.status).toBe('closed')
  })

  it('reconnects with a fresh ticket and the latest replay cursor', async () => {
    let cursor = 2
    const ticketProvider = vi
      .fn<() => string>()
      .mockReturnValueOnce('ws://127.0.0.1/events/ticket-one')
      .mockReturnValueOnce('ws://127.0.0.1/events/ticket-two')
    const statuses: string[] = []
    const client = createRunSocket({
      ticketProvider,
      afterSequence: () => cursor,
      webSocketFactory: factory,
      schedule,
      backoff: {
        initialDelayMs: 125,
        factor: 2,
        maxDelayMs: 500,
        maxAttempts: 3,
        jitterRatio: 0,
      },
      onStatus: (status) => statuses.push(status),
    })

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected
    sockets[0]!.remoteClose()

    expect(scheduled).toHaveLength(1)
    expect(scheduled[0]!.delay).toBe(125)
    expect(client.status).toBe('reconnecting')

    cursor = 8
    scheduled[0]!.callback()
    await Promise.resolve()
    await Promise.resolve()

    expect(ticketProvider).toHaveBeenCalledTimes(2)
    expect(sockets[1]!.url).toBe(
      'ws://127.0.0.1/events/ticket-two?after_sequence=8',
    )
    sockets[1]!.open()
    expect(client.status).toBe('connected')
    expect(statuses).toEqual(['connecting', 'connected', 'reconnecting', 'connected'])
  })

  it('permanently closes after a terminal run event', async () => {
    const client = createRunSocket({
      ticketProvider: () => 'ws://127.0.0.1/events/ticket',
      webSocketFactory: factory,
      schedule,
    })
    const terminalEvent: UiEventEnvelope = {
      ...envelope,
      event_id: eventId('event-terminal'),
      sequence: 2,
      kind: { type: 'run_stopped', reason: 'succeeded' },
    }

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected
    sockets[0]!.message(JSON.stringify({ type: 'event', event: terminalEvent }))

    expect(sockets[0]!.closeCalls).toBe(1)
    expect(scheduled).toEqual([])
    expect(client.status).toBe('closed')
  })

  it('enters a fatal state when the reconnect budget is exhausted', async () => {
    const errors: Error[] = []
    const client = createRunSocket({
      ticketProvider: () => 'ws://127.0.0.1/events/ticket',
      webSocketFactory: factory,
      schedule,
      backoff: {
        initialDelayMs: 50,
        maxAttempts: 1,
        jitterRatio: 0,
      },
      onError: (error) => errors.push(error),
    })

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected
    sockets[0]!.remoteClose()
    scheduled[0]!.callback()
    await Promise.resolve()
    await Promise.resolve()
    sockets[1]!.remoteClose()

    expect(client.status).toBe('fatal')
    expect(scheduled).toHaveLength(1)
    expect(errors.at(-1)?.message).toContain('reconnect budget exhausted')
  })

  it('refreshes the liveness deadline for every valid frame', async () => {
    const cancelled: unknown[] = []
    const client = createRunSocket({
      ticketProvider: () => 'ws://127.0.0.1/events/ticket',
      webSocketFactory: factory,
      schedule,
      cancelScheduled: (handle) => cancelled.push(handle),
      livenessTimeoutMs: 30_000,
    })

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected

    expect(scheduled.map(({ delay }) => delay)).toEqual([30_000])
    sockets[0]!.message(JSON.stringify({ type: 'event', event: envelope }))

    expect(cancelled).toEqual([1])
    expect(scheduled.map(({ delay }) => delay)).toEqual([30_000, 30_000])
  })

  it('closes a stale connection and enters bounded reconnect', async () => {
    const errors: Error[] = []
    const client = createRunSocket({
      ticketProvider: () => 'ws://127.0.0.1/events/ticket',
      webSocketFactory: factory,
      schedule,
      livenessTimeoutMs: 500,
      backoff: {
        initialDelayMs: 100,
        maxAttempts: 2,
        jitterRatio: 0,
      },
      onError: (error) => errors.push(error),
    })

    const connected = client.connect()
    await Promise.resolve()
    sockets[0]!.open()
    await connected
    scheduled[0]!.callback()

    expect(sockets[0]!.closeCalls).toBe(1)
    expect(client.status).toBe('reconnecting')
    expect(scheduled.map(({ delay }) => delay)).toEqual([500, 100])
    expect(errors.at(-1)?.message).toContain('liveness timeout')
  })
})
