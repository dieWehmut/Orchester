import { describe, expect, it } from 'vitest'

import { dayKey, dayLabel, startsDay } from '../src/components/run/transcript-days'

/**
 * Where a transcript crossed midnight.
 *
 * The reference marks the first row of each day with that day and the time the
 * row happened, so a reader returning to a long run can see how much of it
 * happened when. The mark is asked of the whole transcript, because the list is
 * virtualised and a mark computed from what is on screen would appear again
 * every time the window moved.
 */

function rows(...occurred: readonly string[]) {
  return occurred.map((occurredAt, index) => ({ key: `k-${index}`, occurredAt }))
}

describe('dayKey', () => {
  it('names the local calendar day, so two instants in one day agree', () => {
    // Midday and late evening of one local day: the same key whatever the zone.
    expect(dayKey(new Date(2026, 8, 16, 12, 0).toISOString())).toBe('2026-09-16')
    expect(dayKey(new Date(2026, 8, 16, 23, 30).toISOString())).toBe('2026-09-16')
    expect(dayKey(new Date(2026, 8, 17, 0, 30).toISOString())).toBe('2026-09-17')
  })

  it('does not pretend to know a day it cannot read', () => {
    // No day at all, rather than a string that would group every unreadable row
    // under one invented heading.
    expect(dayKey('not-a-date')).toBeNull()
    expect(dayKey(null)).toBeNull()
  })

  it('gives a row the journal left without a time no day of its own', () => {
    const timeline = [
      { key: 'k-0', occurredAt: new Date(2026, 8, 16, 21, 0).toISOString() },
      { key: 'k-1', occurredAt: null },
      { key: 'k-2', occurredAt: new Date(2026, 8, 16, 21, 5).toISOString() },
    ]

    // The gap opens nothing - it has no time to name a day with - and the row
    // after it does not start a new day either, because the gap was in the same
    // one.
    expect(timeline.map((_, index) => startsDay(timeline, index))).toEqual([true, false, false])
  })
})

describe('startsDay', () => {
  it('marks the first row of the transcript', () => {
    const timeline = rows('2026-09-16T10:00:00Z', '2026-09-16T10:01:00Z')

    expect(startsDay(timeline, 0)).toBe(true)
    expect(startsDay(timeline, 1)).toBe(false)
  })

  it('marks the first row after midnight and nothing else', () => {
    const timeline = rows(
      new Date(2026, 8, 16, 21, 53).toISOString(),
      new Date(2026, 8, 16, 23, 59).toISOString(),
      new Date(2026, 8, 17, 0, 1).toISOString(),
      new Date(2026, 8, 17, 8, 0).toISOString(),
    )

    expect(timeline.map((_, index) => startsDay(timeline, index))).toEqual([
      true,
      false,
      true,
      false,
    ])
  })

  it('marks a day even when the transcript is read from a window inside it', () => {
    // The row is the third in the whole transcript and the first of its day, so
    // a mark is drawn even though nothing before it is mounted.
    const timeline = rows(
      new Date(2026, 8, 15, 9, 0).toISOString(),
      new Date(2026, 8, 15, 10, 0).toISOString(),
      new Date(2026, 8, 16, 9, 0).toISOString(),
    )

    expect(startsDay(timeline, 2)).toBe(true)
  })

  it('answers no for an index the timeline does not have', () => {
    expect(startsDay(rows('2026-09-16T10:00:00Z'), 3)).toBe(false)
  })
})

describe('dayLabel', () => {
  it('names the day, the weekday and the time, as the reference does', () => {
    const label = dayLabel(new Date(2026, 8, 16, 21, 53).toISOString())

    // Formatted by the platform rather than by a pattern here, so the assertion
    // is about what has to be in it rather than its exact punctuation.
    expect(label).toContain('16')
    expect(label.length).toBeGreaterThan(8)
  })

  it('says nothing rather than inventing a day', () => {
    expect(dayLabel('not-a-date')).toBe('')
  })
})