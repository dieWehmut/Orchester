import { describe, expect, it } from 'vitest'

import { happyPathFixture, parseUiEventEnvelope, type UiEventEnvelope } from '../../src/index'

describe('happy path fixture', () => {
  it('contains a complete successful run in sequence order', () => {
    const events = happyPathFixture()

    expect(events.map((event) => event.sequence)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
    expect(events.at(-1)?.kind).toEqual({ type: 'run_stopped', reason: 'succeeded' })
    expect(events.every((event: UiEventEnvelope) => parseUiEventEnvelope(event) !== null)).toBe(true)
  })
})
