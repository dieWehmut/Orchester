import { beforeEach, describe, expect, it, vi } from 'vitest'

import { eventId, runId, type UiEventEnvelope } from '@orchester/protokoll'
import { createRunSocket, type WebSocketLike } from '../src/transport/run-socket'

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

  beforeEach(() => {
    sockets = []
    factory = vi.fn((url: string) => {
      const socket = new FakeWebSocket(url)
      sockets.push(socket)
      return socket
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
})
