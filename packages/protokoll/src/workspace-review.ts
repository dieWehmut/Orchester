/**
 * TypeScript mirror of the read-only workspace review served by
 * `kisten/netz` at `GET /api/v1/workspace/review`.
 *
 * This is the data behind three of the Review tab's five filters. The runtime
 * reads it from the working copy, so a view never invents a staging state: the
 * design spec's principle P8 makes that rule explicit.
 *
 * Paths are always relative to the workspace. A path that is absolute, or that
 * names a home directory, is not a path this contract carries.
 */

export const WORKSPACE_REVIEW_SCHEMA_VERSION = 1 as const

/** A path's change, in the vocabulary the diff decorations render. */
export type WorkspaceReviewKind = 'added' | 'modified' | 'deleted' | 'untracked'

export const WORKSPACE_REVIEW_KINDS = [
  'added',
  'modified',
  'deleted',
  'untracked',
] as const satisfies readonly WorkspaceReviewKind[]

/**
 * One changed path and where the change sits.
 *
 * `staged` and `unstaged` are independent: staging a file and then editing it
 * again leaves one path with work in both places, and the two filters have to
 * find it in each.
 */
export interface WorkspaceReviewChangeDto {
  path: string
  kind: WorkspaceReviewKind
  staged: boolean
  unstaged: boolean
}

export interface WorkspaceReviewDto {
  schema_version: typeof WORKSPACE_REVIEW_SCHEMA_VERSION
  /** The branch HEAD points at, or null when HEAD is detached. */
  branch: string | null
  changes: WorkspaceReviewChangeDto[]
  /** Paths this branch changed over its upstream; empty when it has none. */
  branch_changes: string[]
}
