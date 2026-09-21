import { describe, expect, it } from 'vitest'

import {
  FIXTURE_MANIFEST,
  FIXTURE_SCENARIO_IDS,
  fixtureScenarioEvents,
} from '../../src/index'

describe('fixture manifest', () => {
  it('publishes every canonical scenario exactly once', () => {
    expect(FIXTURE_SCENARIO_IDS).toEqual(['happy', 'approval', 'failure', 'reconnect'])
    expect(FIXTURE_MANIFEST.map((entry) => entry.id)).toEqual(FIXTURE_SCENARIO_IDS)
    expect(new Set(FIXTURE_MANIFEST.map((entry) => entry.id))).toHaveLength(
      FIXTURE_MANIFEST.length,
    )
  })

  it('derives deterministic metadata from gap-free event streams', () => {
    for (const entry of FIXTURE_MANIFEST) {
      const events = fixtureScenarioEvents(entry.id)
      expect(events.map((event) => event.sequence), entry.id).toEqual(
        Array.from({ length: events.length }, (_, index) => index + 1),
      )
      expect(entry.event_count, entry.id).toBe(events.length)
      expect(entry.first_sequence, entry.id).toBe(1)
      expect(entry.last_sequence, entry.id).toBe(events.length)
      const terminal = [...events]
        .reverse()
        .find((event) => event.kind.type === 'run_stopped')
      expect(entry.terminal_reason, entry.id).toBe(
        terminal?.kind.type === 'run_stopped' ? terminal.kind.reason : undefined,
      )
    }
  })

  it('is JSON-safe and returns fresh scenario arrays', () => {
    expect(() => JSON.stringify(FIXTURE_MANIFEST)).not.toThrow()

    const first = fixtureScenarioEvents('happy')
    const second = fixtureScenarioEvents('happy')
    expect(first).toEqual(second)
    expect(first).not.toBe(second)
    expect(first[0]).not.toBe(second[0])
  })
})
