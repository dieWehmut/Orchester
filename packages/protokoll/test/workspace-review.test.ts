import { describe, expect, it } from 'vitest'

import {
  WORKSPACE_REVIEW_SCHEMA_VERSION,
  parseWorkspaceReview,
  type WorkspaceReviewDto,
} from '../src/index'

/**
 * The Review tab's change data, task U5-01 of the implementation plan.
 *
 * Section 4.7 asks the tab for five filters. Three of them - staged, unstaged
 * and branch - are facts about the working copy, so the runtime reads them and
 * this mirrors what it serves. 'Last turn' is deliberately not here: which turn
 * a change belongs to is a fact about a run's event stream, not about git.
 */

const review: WorkspaceReviewDto = {
  schema_version: WORKSPACE_REVIEW_SCHEMA_VERSION,
  branch: 'feat/workspace-review-runtime',
  changes: [
    { path: 'src/a.ts', kind: 'added', staged: true, unstaged: false },
    { path: 'src/b.ts', kind: 'modified', staged: false, unstaged: true },
  ],
  branch_changes: ['src/on-branch.ts'],
}

describe('workspace review DTO', () => {
  it('carries no absolute path', () => {
    expect(JSON.stringify(review)).not.toContain('C:\\')
    expect(review.changes.every((change) => !change.path.startsWith('/'))).toBe(true)
  })

  it('keeps staged and unstaged as independent facts', () => {
    const both = parseWorkspaceReview({
      ...review,
      changes: [{ path: 'src/both.ts', kind: 'modified', staged: true, unstaged: true }],
    })
    expect(both?.changes[0]?.staged).toBe(true)
    expect(both?.changes[0]?.unstaged).toBe(true)
  })
})

describe('parseWorkspaceReview', () => {
  it('accepts the served payload', () => {
    expect(parseWorkspaceReview(review)).toEqual(review)
  })

  it('accepts a detached HEAD as a null branch', () => {
    const detached = parseWorkspaceReview({ ...review, branch: null })
    expect(detached?.branch).toBeNull()
  })

  it('rejects an unknown schema version rather than guessing', () => {
    expect(parseWorkspaceReview({ ...review, schema_version: 99 })).toBeNull()
  })

  it('rejects an unknown change kind rather than rendering it', () => {
    expect(
      parseWorkspaceReview({
        ...review,
        changes: [{ path: 'src/a.ts', kind: 'renamed', staged: true, unstaged: false }],
      }),
    ).toBeNull()
  })

  it('rejects a payload whose flags are not booleans', () => {
    expect(
      parseWorkspaceReview({
        ...review,
        changes: [{ path: 'src/a.ts', kind: 'added', staged: 'yes', unstaged: false }],
      }),
    ).toBeNull()
  })

  it('rejects an absolute path, because the browser must never receive one', () => {
    expect(
      parseWorkspaceReview({
        ...review,
        changes: [{ path: 'C:\\project\\secret.ts', kind: 'added', staged: true, unstaged: false }],
      }),
    ).toBeNull()
  })

  it('rejects extra keys rather than passing unknown state through', () => {
    expect(parseWorkspaceReview({ ...review, workspace_root: '/home/user' })).toBeNull()
  })
})
