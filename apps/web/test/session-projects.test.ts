import type { SessionSummaryDto } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { groupByProject } from '../src/components/sessions/session-projects'

/**
 * The rail's sessions, grouped by the project they ran in.
 *
 * The reference's sidebar is a list of projects with their conversations nested
 * under them. The project is the name of the directory the runtime recorded for
 * the run, so it is a fact about the session rather than a label somebody has to
 * maintain - and a session whose record named none is not filed under a project
 * it never ran in.
 */

function session(id: string, project?: string | null): SessionSummaryDto {
  return {
    id,
    source: 'delegate',
    recorded_at_unix: 1_700_000_000,
    title: `run ${id}`,
    agent: 'codex',
    model: 'gpt-5',
    outcome: 'success',
    resumable: false,
    ...(project === undefined ? {} : { project }),
  }
}

describe('groupByProject', () => {
  it('files each run under the project it ran in, newest project first', () => {
    const groups = groupByProject([
      session('a', 'Nexus'),
      session('b', 'Orchester'),
      session('c', 'Nexus'),
    ])

    // The order is the order the sessions arrived in, which is newest first: the
    // project a reader touched most recently is the one at the top.
    expect(groups.map((group) => group.project)).toEqual(['Nexus', 'Orchester'])
    expect(groups[0]!.items.map((item) => item.id)).toEqual(['a', 'c'])
    expect(groups[1]!.items.map((item) => item.id)).toEqual(['b'])
  })

  it('keeps the runs that named no project, without inventing one for them', () => {
    const groups = groupByProject([
      session('a'),
      session('b', 'Nexus'),
      session('c', null),
    ])

    expect(groups.map((group) => group.project)).toEqual(['Nexus', null])
    expect(groups[1]!.items.map((item) => item.id)).toEqual(['a', 'c'])
  })

  it('puts the unfiled runs last, whatever order they arrived in', () => {
    // A group of runs the rail cannot file, above the real projects, would be
    // the first thing a reader reads.
    const groups = groupByProject([session('a'), session('b', 'Nexus')])

    expect(groups[groups.length - 1]!.project).toBeNull()
  })

  it('has nothing to say about nothing', () => {
    expect(groupByProject([])).toEqual([])
  })
})