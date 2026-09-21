import { describe, expect, it } from 'vitest'

import { parseUiEventEnvelope, reconnectPathFixture } from '../../src/index'

describe('reconnect path fixture', () => {
  it('contains an intentional replay overlap for sequence deduplication', () => {
    const fixture = reconnectPathFixture()
    const before = new Set(fixture.before_disconnect.map((event) => event.sequence))
    const replayed = fixture.replay_response.events.map((event) => event.sequence)

    expect(fixture.replay_request.after_sequence).toBe(3)
    expect(replayed.filter((sequence) => before.has(sequence))).toEqual([4, 5])
  })

  it('reconstructs one ordered run when keyed by sequence', () => {
    const fixture = reconnectPathFixture()
    const bySequence = new Map(
      [...fixture.before_disconnect, ...fixture.replay_response.events].map((event) => [
        event.sequence,
        event,
      ]),
    )

    expect([...bySequence.keys()].sort((left, right) => left - right)).toEqual([
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ])
    expect([...bySequence.values()].every((event) => parseUiEventEnvelope(event) !== null)).toBe(
      true,
    )
  })
})
