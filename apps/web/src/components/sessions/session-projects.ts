import type { SessionSummaryDto } from '@orchester/protokoll'

/**
 * The rail's sessions, grouped by the project they ran in.
 *
 * The reference's sidebar is a list of projects with their conversations nested
 * under them, which is what a reader returning to the rail is looking for:
 * "where is that thing I did in Nexus?" rather than "what did I run on Tuesday?".
 * The project is the name of the directory the runtime recorded for the run, so
 * it is a fact rather than a label somebody has to maintain.
 *
 * A session whose record named no project - one written before the runtime
 * carried the directory, or from a relative one - is grouped under `null` and
 * drawn without a heading of its own, because inventing one would put a session
 * under a project it never ran in.
 */
export interface SessionProjectGroup {
  /** The project's name, or null for the sessions that named none. */
  readonly project: string | null
  readonly items: readonly SessionSummaryDto[]
}

export function groupByProject(
  items: readonly SessionSummaryDto[],
): readonly SessionProjectGroup[] {
  const groups: { project: string | null; items: SessionSummaryDto[] }[] = []
  const byName = new Map<string | null, { project: string | null; items: SessionSummaryDto[] }>()
  for (const item of items) {
    const project = item.project ?? null
    let group = byName.get(project)
    if (group === undefined) {
      // The order is the order the sessions arrived in - newest first, as the
      // runtime pages them - so the projects a reader touched most recently are
      // the ones at the top, which is the order the reference's rail has too.
      group = { project, items: [] }
      byName.set(project, group)
      groups.push(group)
    }
    group.items.push(item)
  }
  // The sessions that named no project come last: they are the ones the rail
  // cannot file, and a group of them above the real projects would be the first
  // thing a reader reads.
  const unfiled = groups.filter((group) => group.project === null)
  return [...groups.filter((group) => group.project !== null), ...unfiled]
}