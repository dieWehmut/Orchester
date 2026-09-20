import type { FileChangeTimelineItem, TurnView } from '@orchester/ereignis'
import { turnId } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import {
  REVIEW_FILTERS,
  filterChanges,
  turnIndexByPath,
  withTurns,
  type ReviewChange,
} from '../src/components/changes/review-filters'
import ReviewPanel from '../src/components/changes/ReviewPanel.vue'

/**
 * The Review tab's change filters, task U5-01 of the implementation plan.
 *
 * Section 4.7 asks the Review tab for five filters and a file list ordered the
 * way the diff list is ordered. The filter rules live here rather than in the
 * component because which changes a filter admits is what the reader is
 * actually choosing, and it is decidable without a rendered tree.
 *
 * Four of the five are facts the runtime serves; 'last turn' is the join of
 * that set with the run's own event stream, so it is the one filter the caller
 * has to supply a turn for.
 */

function change(path: string, staged: boolean, unstaged: boolean, turn: number | null = 1): ReviewChange {
  return {
    path,
    kind: 'modified' as const,
    staged,
    unstaged,
    turn,
  }
}

const changes: readonly ReviewChange[] = [
  change('src/a.ts', true, false, 1),
  change('src/b.ts', false, true, 1),
  change('src/c.ts', false, true, 2),
]

const context = { branchChanges: ['src/b.ts'], lastTurn: 2 }

describe('review filters', () => {
  it('names the five filters the spec asks for', () => {
    expect(REVIEW_FILTERS).toEqual(['all', 'staged', 'unstaged', 'branch', 'last-turn'])
  })

  it('admits everything under All', () => {
    expect(filterChanges(changes, 'all', context)).toHaveLength(3)
  })

  it('admits staged work under Staged', () => {
    expect(filterChanges(changes, 'staged', context).map((entry) => entry.path)).toEqual([
      'src/a.ts',
    ])
  })

  it('admits work not in the index under Unstaged', () => {
    expect(filterChanges(changes, 'unstaged', context).map((entry) => entry.path)).toEqual([
      'src/b.ts',
      'src/c.ts',
    ])
  })

  it('admits a path that is staged and edited again under both filters', () => {
    // Staging a file and then editing it again leaves one path with work in
    // the index and different work on disk. Both filters exist precisely so
    // the reader can find it from either side, so neither may hide it.
    const both = [change('src/both.ts', true, true)]

    expect(filterChanges(both, 'staged', context)).toHaveLength(1)
    expect(filterChanges(both, 'unstaged', context)).toHaveLength(1)
  })

  it('admits the paths the branch changed over its upstream', () => {
    // 'Branch' means the work committed here, which is a different set from
    // the working copy's staged and unstaged changes.
    expect(filterChanges(changes, 'branch', context).map((entry) => entry.path)).toEqual([
      'src/b.ts',
    ])
  })

  it('admits only the last turn under Last turn', () => {
    expect(filterChanges(changes, 'last-turn', context).map((entry) => entry.path)).toEqual([
      'src/c.ts',
    ])
  })

  it('admits nothing under Last turn when the run has no turn yet', () => {
    // A run that has not started has no last turn. Guessing one would show
    // the reader changes that belong to no turn at all.
    expect(filterChanges(changes, 'last-turn', { ...context, lastTurn: null })).toHaveLength(0)
  })

  it('admits nothing rather than everything when the branch has no upstream', () => {
    // An unpublished branch has no committed work to show. Falling back to
    // the working copy would answer a question nobody asked.
    expect(filterChanges(changes, 'branch', { ...context, branchChanges: [] })).toHaveLength(0)
  })

  it('orders the file list the way the diff list is ordered', () => {
    // Section 4.7 requires the two orders to match, so the filter may not
    // reshuffle what it admits.
    const paths = filterChanges(
      [change('src/z.ts', false, true), change('src/a.ts', false, true)],
      'unstaged',
      context,
    ).map((entry) => entry.path)

    expect(paths).toEqual(['src/z.ts', 'src/a.ts'])
  })
})

describe('ReviewPanel', () => {
  it('reports the active filter it starts on', () => {
    const wrapper = mount(ReviewPanel, { props: { changes, branchChanges: ['src/b.ts'], lastTurn: 2 } })

    expect(wrapper.get('[data-review-panel]').attributes('data-review-filter')).toBe('all')
    expect(wrapper.findAll('[data-review-file]')).toHaveLength(3)
  })

  it('narrows the list when a filter is chosen', async () => {
    const wrapper = mount(ReviewPanel, { props: { changes, branchChanges: ['src/b.ts'], lastTurn: 2 } })

    const staged = wrapper
      .findAll('[role="radio"]')
      .find((option) => option.text() === 'Staged')
    await staged?.trigger('click')

    expect(wrapper.get('[data-review-panel]').attributes('data-review-filter')).toBe('staged')
    expect(wrapper.findAll('[data-review-file]')).toHaveLength(1)
  })

  it('says so when a filter admits nothing, rather than showing an empty box', async () => {
    const wrapper = mount(ReviewPanel, {
      props: { changes: [change('src/a.ts', true, false)], branchChanges: [], lastTurn: null },
    })

    const branch = wrapper
      .findAll('[role="radio"]')
      .find((option) => option.text() === 'Branch')
    await branch?.trigger('click')

    expect(wrapper.find('[data-review-empty]').exists()).toBe(true)
  })
})

describe('withTurns', () => {
  it('attaches the turn the run reported for a path', () => {
    const served = [
      { path: 'src/a.ts', kind: 'modified' as const, staged: true, unstaged: false },
      { path: 'src/b.ts', kind: 'added' as const, staged: false, unstaged: true },
    ]

    const joined = withTurns(served, new Map([['src/b.ts', 4]]))

    expect(joined.map((change) => change.turn)).toEqual([null, 4])
  })

  it('leaves a path the run never reported belonging to no turn', () => {
    // The working copy can hold changes from before this run, or from outside
    // it entirely. Assigning them a turn would put them under Last turn, which
    // is a claim the run cannot support.
    const served = [
      { path: 'src/outside.ts', kind: 'modified' as const, staged: false, unstaged: true },
    ]

    const joined = withTurns(served, new Map())

    expect(joined[0]?.turn).toBeNull()
    expect(filterChanges(joined, 'last-turn', { branchChanges: [], lastTurn: 2 })).toHaveLength(0)
  })

  it('keeps the staged and unstaged placement the runtime reported', () => {
    const served = [
      { path: 'src/a.ts', kind: 'modified' as const, staged: true, unstaged: true },
    ]

    const joined = withTurns(served, new Map([['src/a.ts', 1]]))

    expect(joined[0]).toMatchObject({ staged: true, unstaged: true, turn: 1 })
  })
})

describe('turnIndexByPath', () => {
  it('numbers the turns from one in the order the run reported them', () => {
    const turns = [
      { key: 't1', id: turnId('turn-1'), startedAt: null, endedAt: null, items: [] },
      { key: 't2', id: turnId('turn-2'), startedAt: null, endedAt: null, items: [] },
    ] as unknown as TurnView[]
    const fileChanges = [
      changeEvent('src/a.ts', turnId('turn-1')),
      changeEvent('src/b.ts', turnId('turn-2')),
    ]

    const index = turnIndexByPath(turns, fileChanges)

    expect(index.get('src/a.ts')).toBe(1)
    expect(index.get('src/b.ts')).toBe(2)
  })

  it('leaves out a change whose turn the projection never numbered', () => {
    // A gap or a change that arrived with no turn cannot be placed. Guessing
    // would file it under Last turn, which is a different claim.
    const turns = [
      { key: 't1', id: turnId('turn-1'), startedAt: null, endedAt: null, items: [] },
    ] as unknown as TurnView[]
    const fileChanges = [
      changeEvent('src/orphan.ts', turnId('turn-missing')),
      changeEvent('src/unturned.ts', null),
    ]

    const index = turnIndexByPath(turns, fileChanges)

    expect(index.has('src/orphan.ts')).toBe(false)
    expect(index.has('src/unturned.ts')).toBe(false)
  })

  it('feeds Last turn the newest turn the run reported', () => {
    const turns = [
      { key: 't1', id: turnId('turn-1'), startedAt: null, endedAt: null, items: [] },
      { key: 't2', id: turnId('turn-2'), startedAt: null, endedAt: null, items: [] },
    ] as unknown as TurnView[]
    const fileChanges = [
      changeEvent('src/old.ts', turnId('turn-1')),
      changeEvent('src/new.ts', turnId('turn-2')),
    ]
    const served = [
      { path: 'src/old.ts', kind: 'modified' as const, staged: true, unstaged: false },
      { path: 'src/new.ts', kind: 'modified' as const, staged: false, unstaged: true },
    ]

    const joined = withTurns(served, turnIndexByPath(turns, fileChanges))
    const admitted = filterChanges(joined, 'last-turn', { branchChanges: [], lastTurn: 2 })

    expect(admitted.map((entry) => entry.path)).toEqual(['src/new.ts'])
  })
})

function changeEvent(path: string, turn: ReturnType<typeof turnId> | null): FileChangeTimelineItem {
  return {
    type: 'file_change',
    key: `change:${path}` ,
    sequence: 1,
    occurredAt: '2026-09-20T00:00:00Z',
    turnId: turn,
    path,
    kind: 'update',
  } as FileChangeTimelineItem
}
