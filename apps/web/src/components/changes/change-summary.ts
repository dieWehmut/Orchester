import type { FileChangeTimelineItem } from '@orchester/ereignis'

export interface ChangeSummary {
  readonly path: string
  readonly kind: FileChangeTimelineItem['kind']
  readonly latestSequence: number
  readonly latestOccurredAt: string
  readonly eventCount: number
  readonly history: readonly FileChangeTimelineItem[]
}

export function summarizeFileChanges(
  changes: readonly FileChangeTimelineItem[],
): ChangeSummary[] {
  const byPath = new Map<string, FileChangeTimelineItem[]>()

  for (const change of changes) {
    const history = byPath.get(change.path)
    if (history) history.push(change)
    else byPath.set(change.path, [change])
  }

  return [...byPath.entries()]
    .map(([path, unsortedHistory]) => {
      const history = [...unsortedHistory].sort((left, right) => left.sequence - right.sequence)
      const latest = history[history.length - 1]
      if (!latest) throw new Error(`change history is empty for ${path}`)

      return {
        path,
        kind: latest.kind,
        latestSequence: latest.sequence,
        latestOccurredAt: latest.occurredAt,
        eventCount: history.length,
        history,
      }
    })
    .sort(
      (left, right) =>
        right.latestSequence - left.latestSequence || left.path.localeCompare(right.path),
    )
}
