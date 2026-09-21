import {
  WORKSPACE_REVIEW_KINDS,
  WORKSPACE_REVIEW_SCHEMA_VERSION,
  type WorkspaceReviewChangeDto,
  type WorkspaceReviewDto,
  type WorkspaceReviewKind,
} from './workspace-review'

type UnknownRecord = Record<string, unknown>

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(record: UnknownRecord, allowed: readonly string[]): boolean {
  return Object.keys(record).every((key) => allowed.includes(key))
}

/**
 * A workspace-relative path, or null.
 *
 * The workspace root must never reach the browser (rule 1 of the frontends
 * plan), so an absolute path is rejected rather than normalized: a payload that
 * carries one is a payload this contract does not describe, and quietly
 * stripping it would hide the bug that produced it.
 */
function relativePath(value: unknown): string | null {
  if (typeof value !== 'string') return null
  if (!value || value.length > 1024) return null
  if (/[\u0000-\u001f\u007f]/.test(value)) return null
  if (value.startsWith('/') || value.startsWith('\\')) return null
  if (/^[A-Za-z]:/.test(value)) return null
  if (value.split(/[\\/]/).includes('..')) return null
  return value
}

function changeKind(value: unknown): WorkspaceReviewKind | null {
  return typeof value === 'string' && WORKSPACE_REVIEW_KINDS.includes(value as WorkspaceReviewKind)
    ? (value as WorkspaceReviewKind)
    : null
}

function parseChange(raw: unknown): WorkspaceReviewChangeDto | null {
  if (!isRecord(raw) || !hasOnlyKeys(raw, ['path', 'kind', 'staged', 'unstaged'])) return null

  const path = relativePath(raw.path)
  const kind = changeKind(raw.kind)
  if (path === null || kind === null) return null
  if (typeof raw.staged !== 'boolean' || typeof raw.unstaged !== 'boolean') return null

  return { path, kind, staged: raw.staged, unstaged: raw.unstaged }
}

/** Parse the served review, or null when the payload is not this contract. */
export function parseWorkspaceReview(raw: unknown): WorkspaceReviewDto | null {
  if (
    !isRecord(raw) ||
    !hasOnlyKeys(raw, ['schema_version', 'branch', 'changes', 'branch_changes']) ||
    raw.schema_version !== WORKSPACE_REVIEW_SCHEMA_VERSION
  ) {
    return null
  }

  const branch = raw.branch === null ? null : relativePath(raw.branch)
  if (raw.branch !== null && branch === null) return null

  if (!Array.isArray(raw.changes) || !Array.isArray(raw.branch_changes)) return null

  const changes: WorkspaceReviewChangeDto[] = []
  for (const entry of raw.changes) {
    const change = parseChange(entry)
    if (change === null) return null
    changes.push(change)
  }

  const branchChanges: string[] = []
  for (const entry of raw.branch_changes) {
    const path = relativePath(entry)
    if (path === null) return null
    branchChanges.push(path)
  }

  return {
    schema_version: WORKSPACE_REVIEW_SCHEMA_VERSION,
    branch,
    changes,
    branch_changes: branchChanges,
  }
}

export function parseWorkspaceReviewJson(text: string): WorkspaceReviewDto | null {
  try {
    return parseWorkspaceReview(JSON.parse(text) as unknown)
  } catch {
    return null
  }
}
