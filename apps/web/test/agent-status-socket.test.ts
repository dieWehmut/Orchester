import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
  AGENT_FLEET_FIXTURE,
  type AgentFleetStreamFrameDto,
} from '@orchester/protokoll'

import {
  createAgentStatusSocket,
  type AgentStatusSocket,
} from '../src/transport/agent-status-socket'
import type { WebSocketLike } from '../src/transport/run-socket'

interface ScheduledTask {
  callback: () => void
  delay: number
}

class FakeWebSocket implements WebSocketLike {
  readonly url: string
  readonly sent: string[] = []
  readyState = 0
  onopen: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onclose: ((event: CloseEvent) => void) | null = null
  onerror: ((event: Event) => void) | null = null

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
    this.readyState = 3
    this.onclose?.({ code: 1000, wasClean: true } as CloseEvent)
  }
}

describe('agent status socket lifecycle', () => {
  let sockets: FakeWebSocket[]
  let scheduled: ScheduledTask[]
  let factory: (url: string) => WebSocketLike
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

  it('dispatches validated snapshots and heartbeats without exposing parser details', async () => {
    const snapshots: unknown[] = []
    const heartbeats: unknown[] = []
    const errors: Error[] = []
    const client = createAgentStatusSocket({
      urlProvider: () => 'ws://127.0.0.1/api/v1/agents/status/ws',
      webSocketFactory: factory,
      onSnapshot: (snapshot) => snapshots.push(snapshot),
      onHeartbeat: (heartbeat) => heartbeats.push(heartbeat),
      onError: (error) => errors.push(error),
    })

    const connected = client.connect()
    await Promise.resolve()
    expect(sockets[0]?.url).toBe('ws://127.0.0.1/api/v1/agents/status/ws')
    sockets[0]!.open()
    await connected

    const snapshotFrame: AgentFleetStreamFrameDto = {
      type: 'snapshot',
      snapshot: AGENT_FLEET_FIXTURE,
    }
    sockets[0]!.message(JSON.stringify(snapshotFrame))
    sockets[0]!.message(
      JSON.stringify({
        type: 'heartbeat',
        sequence: 12,
        sent_at: '2026-08-20T08:10:00.000Z',
      }),
    )
    sockets[0]!.message('{not-json')

    expect(snapshots).toEqual([AGENT_FLEET_FIXTURE])
    expect(heartbeats).toEqual([
      { type: 'heartbeat', sequence: 12, sent_at: '2026-08-20T08:10:00.000Z' },
    ])
    expect(errors).toHaveLength(1)
    expect(errors[0]?.message).toContain('Invalid agent status stream frame')
  })

  it('reconnects with a fresh server URL after a remote close', async () => {
    const urls = ['ws://127.0.0.1/one', 'ws://127.0.0.1/two']
    const urlProvider = vi.fn(() => urls.shift() ?? 'ws://127.0.0.1/fallback')
    const statuses: string[] = []
    const client = createAgentStatusSocket({
      urlProvider,
      webSocketFactory: factory,
      schedule,
      backoff: { initialDelayMs: 125, maxAttempts: 2, jitterRatio: 0 },
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

    scheduled[0]!.callback()
    await Promise.resolve()
    await Promise.resolve()
    expect(urlProvider).toHaveBeenCalledTimes(2)
    expect(sockets[1]!.url).toBe('ws://127.0.0.1/two')
    sockets[1]!.open()
    expect(client.status).toBe('connected')
    expect(statuses).toEqual(['connecting', 'connected', 'reconnecting', 'connected'])
  })

  it('does not open a socket after close while the URL is pending', async () => {
    let resolveUrl!: (url: string) => void
    const pendingUrl = new Promise<string>((resolve) => {
      resolveUrl = resolve
    })
    const client: AgentStatusSocket = createAgentStatusSocket({
      urlProvider: () => pendingUrl,
      webSocketFactory: factory,
    })

    const connected = client.connect()
    client.close()
    resolveUrl('ws://127.0.0.1/late')

    await expect(connected).rejects.toThrow('Agent status socket is closed')
    expect(sockets).toEqual([])
    expect(client.status).toBe('closed')
  })
})
