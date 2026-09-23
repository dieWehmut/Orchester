import { beforeEach, describe, expect, it } from 'vitest'

import {
  PINNED_SESSIONS_STORAGE_KEY,
  parsePinnedSessions,
  resetPinnedSessionsForTests,
  usePinnedSessions,
} from '../src/composables/use-pinned-sessions'

/**
 * The reader's own ordering of their runs.
 *
 * The reference's sidebar opens with a pinned list. Nothing about a run changes
 * when it is pinned, so this is a reading preference kept on the machine - and a
 * stored value is not trusted, because a key a reader has edited by hand must
 * open the rail rather than break it.
 */
describe('pinned sessions', () => {
  beforeEach(() => {
    localStorage.clear()
    resetPinnedSessionsForTests()
  })

  it('starts empty and pins the newest run at the top', () => {
    const { pinned, toggle, isPinned } = usePinnedSessions()

    expect(pinned.value).toEqual([])

    toggle('s-1')
    toggle('s-2')

    expect(pinned.value).toEqual(['s-2', 's-1'])
    expect(isPinned('s-1')).toBe(true)
    expect(JSON.parse(localStorage.getItem(PINNED_SESSIONS_STORAGE_KEY) ?? '[]')).toEqual([
      's-2',
      's-1',
    ])
  })

  it('unpins what is already pinned', () => {
    const { pinned, toggle } = usePinnedSessions()

    toggle('s-1')
    toggle('s-2')
    toggle('s-1')

    expect(pinned.value).toEqual(['s-2'])
  })

  it('keeps only what could be an identifier', () => {
    expect(parsePinnedSessions(null)).toEqual([])
    expect(parsePinnedSessions('not json')).toEqual([])
    expect(parsePinnedSessions('{"a":1}')).toEqual([])
    expect(parsePinnedSessions('["s-1", 7, "", "  ", "s-1", " s-2 "]')).toEqual(['s-1', 's-2'])
  })

  it('reads what a previous session pinned', () => {
    localStorage.setItem(PINNED_SESSIONS_STORAGE_KEY, JSON.stringify(['s-9']))

    expect(usePinnedSessions().pinned.value).toEqual(['s-9'])
  })
})