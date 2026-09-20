import { describe, expect, it } from 'vitest'

import { cycleTab, reorderTab, tabAt, type ShellTab } from '../src/components/layout/tab-strip'

/**
 * The strip's list rules, kept out of the component so the cycle, the numbered
 * selection and the reorder can each be held to their rule on their own.
 */

const TABS: readonly ShellTab[] = [
  { id: 'a', kind: 'run', label: 'A' },
  { id: 'b', kind: 'terminal', label: 'B' },
  { id: 'c', kind: 'diff', label: 'C' },
]

describe('tab strip list rules', () => {
  it('cycles forwards and backwards, wrapping at both ends', () => {
    expect(cycleTab(TABS, 'a')).toBe('b')
    expect(cycleTab(TABS, 'c')).toBe('a')
    expect(cycleTab(TABS, 'a', true)).toBe('c')
    expect(cycleTab(TABS, 'b', true)).toBe('a')
  })

  it('starts the cycle at the near end when the selection is not in the list', () => {
    expect(cycleTab(TABS, 'gone')).toBe('a')
    expect(cycleTab(TABS, 'gone', true)).toBe('c')
  })

  it('has nothing to cycle when the strip is empty', () => {
    expect(cycleTab([], 'a')).toBeNull()
  })

  it('addresses tabs by their one-based position, and only up to nine', () => {
    expect(tabAt(TABS, 1)).toBe('a')
    expect(tabAt(TABS, 3)).toBe('c')
    expect(tabAt(TABS, 4)).toBeNull()
    expect(tabAt(TABS, 0)).toBeNull()
    expect(tabAt(TABS, 10)).toBeNull()
  })

  it('moves a tab to the dropped position without touching the others', () => {
    expect(reorderTab(TABS, 'c', 'a').map((tab) => tab.id)).toEqual(['c', 'a', 'b'])
    expect(reorderTab(TABS, 'a', 'c').map((tab) => tab.id)).toEqual(['b', 'c', 'a'])
    // A move that is not a move, and ids the strip does not hold, leave it be.
    expect(reorderTab(TABS, 'a', 'a')).toBe(TABS)
    expect(reorderTab(TABS, 'a', 'gone')).toBe(TABS)
  })
})
