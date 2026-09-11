import type { FileChangeTimelineItem } from '@orchester/ereignis'
import { describe, expect, it } from 'vitest'

import { summarizeFileChanges } from '../src/components/changes/change-summary'

function change(
  path: string,
  sequence: number,
  kind: FileChangeTimelineItem['kind'],
): FileChangeTimelineItem {
  return {
    type: 'file_change',
    key: `change-${sequence}-${path}`,
    sequence,
    occurredAt: `2026-08-21T00:00:0${sequence}Z`,
    turnId: null,
    path,
    kind,
  }
}

describe('summarizeFileChanges', () => {
  it('returns no summaries when a run has no file changes', () => {
    expect(summarizeFileChanges([])).toEqual([])
  })

  it('collapses repeated paths to their latest state while retaining history', () => {
    const first = change('src/app.ts', 1, 'add')
    const other = change('src/router.ts', 2, 'delete')
    const latest = change('src/app.ts', 3, 'update')

    expect(summarizeFileChanges([first, other, latest])).toEqual([
      {
        path: 'src/app.ts',
        kind: 'update',
        latestSequence: 3,
        latestOccurredAt: latest.occurredAt,
        eventCount: 2,
        history: [first, latest],
      },
      {
        path: 'src/router.ts',
        kind: 'delete',
        latestSequence: 2,
        latestOccurredAt: other.occurredAt,
        eventCount: 1,
        history: [other],
      },
    ])
  })

  it('uses path order to make equal latest sequences deterministic', () => {
    expect(
      summarizeFileChanges([
        change('src/zeta.ts', 4, 'update'),
        change('src/alpha.ts', 4, 'add'),
      ]).map(({ path }) => path),
    ).toEqual(['src/alpha.ts', 'src/zeta.ts'])
  })
})
