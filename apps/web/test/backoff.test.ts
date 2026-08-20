import { describe, expect, it } from 'vitest'

import { createReconnectBackoff } from '../src/transport/backoff'

describe('run socket reconnect backoff', () => {
  it('grows exponentially, clamps the delay, and exhausts after a finite budget', () => {
    const backoff = createReconnectBackoff({
      initialDelayMs: 100,
      factor: 2,
      maxDelayMs: 350,
      maxAttempts: 4,
      jitterRatio: 0,
    })

    expect(backoff.next()).toBe(100)
    expect(backoff.next()).toBe(200)
    expect(backoff.next()).toBe(350)
    expect(backoff.next()).toBe(350)
    expect(backoff.next()).toBeNull()
    expect(backoff.exhausted).toBe(true)
  })

  it('resets the attempt budget after a successful connection', () => {
    const backoff = createReconnectBackoff({
      initialDelayMs: 80,
      factor: 2,
      maxDelayMs: 500,
      maxAttempts: 2,
      jitterRatio: 0,
    })

    expect(backoff.next()).toBe(80)
    expect(backoff.next()).toBe(160)
    expect(backoff.next()).toBeNull()

    backoff.reset()

    expect(backoff.attempt).toBe(0)
    expect(backoff.next()).toBe(80)
  })

  it('keeps jitter inside the configured bounded range', () => {
    const backoff = createReconnectBackoff({
      initialDelayMs: 100,
      factor: 2,
      maxDelayMs: 100,
      maxAttempts: 1,
      jitterRatio: 0.25,
      random: () => 1,
    })

    expect(backoff.next()).toBe(100)
  })
})
