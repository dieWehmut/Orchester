import type { FileChangeTimelineItem, TurnView } from '@orchester/ereignis'
import type { WorkspaceReviewChangeDto } from '@orchester/protokoll'

/**
 * The Review tab's change filters, task U5-01 of the implementation plan.
 *
 * Section 4.7 asks for filters over `All / Staged / Unstaged / Branch / Last
 * turn`, and for a file list ordered identically to the diff list. The rules
 * live here rather than in the component because which changes a filter admits
 * is what the reader is actually choosing, and it is decidable without a
 * rendered tree.
 *
 * Four of the five filters are facts the runtime serves. `last-turn` is the one
 * that is not: which turn a change belongs to is a fact about a run's event
 * stream, so the caller joins it in and supplies the last turn itself.
 */

export const REVIEW_FILTERS = ['all', 'staged', 'unstaged', 'branch', 'last-turn'] as const

export type ReviewFilter = (typeof REVIEW_FILTERS)[number]

/** A served change, plus the turn the run's event stream placed it in. */
export interface ReviewChange extends WorkspaceReviewChangeDto {
  /** The turn this change arrived in, or null when no run has claimed it. */
  readonly turn: number | null
}

/** What the filters need that the working copy alone cannot answer. */
export interface ReviewFilterContext {
  /** Paths this branch changed over its upstream; empty when it has none. */
  readonly branchChanges: readonly string[]
  /** The newest turn the run reported, or null when it has not started one. */
  readonly lastTurn: number | null
}

/**
 * The changes a filter admits, in the order the diff list is ordered.
 *
 * Order is preserved rather than recomputed: section 4.7 requires the file tree
 * and the diff list to agree, so a filter that reordered its own output would
 * break the one thing the reader is comparing.
 */
export function filterChanges(
  changes: readonly ReviewChange[],
  filter: ReviewFilter,
  context: ReviewFilterContext,
): ReviewChange[] {
  switch (filter) {
    case 'all':
      return [...changes]
    case 'staged':
      // A path staged and then edited again has work in both places, and both
      // filters must find it: neither side is the whole story.
      return changes.filter((change) => change.staged)
    case 'unstaged':
      return changes.filter((change) => change.unstaged)
    case 'branch': {
      // Branch work is the set committed over the upstream, which is not the
      // same set as the working copy's. A branch with no upstream has none, and
      // answering with the working copy instead would be a different question.
      const onBranch = new Set(context.branchChanges)
      return changes.filter((change) => onBranch.has(change.path))
    }
    case 'last-turn':
      // A run that has not started a turn has no last turn. Every change then
      // matches nothing, because guessing a turn would show work that belongs
      // to no turn at all.
      return context.lastTurn === null
        ? []
        : changes.filter((change) => change.turn === context.lastTurn)
  }
}

/**
 * Attach the turn each path was last touched in.
 *
 * The working copy is the authority on where a change sits, and the run is the
 * authority on which turn it arrived in; neither knows the other. So the join is
 * by path: a served change whose path the run reported takes that turn, and a
 * path the run never reported keeps `null` rather than being assigned a turn it
 * cannot be shown to belong to.
 */
export function withTurns(
  changes: readonly WorkspaceReviewChangeDto[],
  turnByPath: ReadonlyMap<string, number>,
): ReviewChange[] {
  return changes.map((change) => ({
    ...change,
    turn: turnByPath.get(change.path) ?? null,
  }))
}

/**
 * The turn each path was last touched in, counted from one.
 *
 * A run reports file changes with the turn they arrived in, and counts turns by
 * their order. Only the run knows this: the working copy knows where a change
 * sits but not which turn produced it. A change whose turn the projection never
 * numbered is left out rather than guessed at, so it cannot fall under Last turn
 * by accident.
 */
export function turnIndexByPath(
  turns: readonly TurnView[],
  fileChanges: readonly FileChangeTimelineItem[],
): Map<string, number> {
  const indexByTurnId = new Map<string, number>()
  turns.forEach((turn, index) => {
    if (turn.id !== null) indexByTurnId.set(turn.id, index + 1)
  })

  const turnByPath = new Map<string, number>()
  for (const change of fileChanges) {
    if (change.turnId === null) continue
    const turn = indexByTurnId.get(change.turnId)
    if (turn === undefined) continue
    turnByPath.set(change.path, turn)
  }
  return turnByPath
}
