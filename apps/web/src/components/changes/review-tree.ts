import type { ReviewChange } from './review-filters'

/**
 * The Review tab's file tree, task U5-02 of the implementation plan.
 *
 * Section 4.7 asks for a file tree ordered identically to the diff list and for
 * expand/collapse navigation. The tree is derived from the flat, already-ordered
 * change list rather than sorted independently, so the two orders cannot drift:
 * flattening the tree gives back the list it came from.
 *
 * That constraint is why a directory's children keep the order of the changes
 * under them instead of being sorted by name, and why a directory appears at the
 * position of its first change rather than where its name would sort.
 */

export interface ReviewTreeFile {
  readonly kind: 'file'
  /** The full workspace-relative path, which is what a change is keyed by. */
  readonly path: string
  readonly name: string
  readonly change: ReviewChange
}

export interface ReviewTreeDirectory {
  readonly kind: 'directory'
  /** The directory path, relative to the workspace. */
  readonly path: string
  readonly name: string
  readonly children: ReviewTreeNode[]
}

export type ReviewTreeNode = ReviewTreeFile | ReviewTreeDirectory

/**
 * Build the tree, preserving the order the change list came in.
 *
 * A path's directories are created on first sight, so a directory's place in the
 * tree is the place of its first change. Files keep the relative order they were
 * given, which is what makes the flattened tree equal to the diff list.
 */
export function buildReviewTree(changes: readonly ReviewChange[]): ReviewTreeNode[] {
  const roots: ReviewTreeNode[] = []
  // Directory lookup by path, so a later change under the same directory finds
  // the node the first one created rather than making a second.
  const directories = new Map<string, ReviewTreeDirectory>()

  for (const change of changes) {
    const segments = change.path.split('/').filter((segment) => segment.length > 0)
    const fileName = segments.pop()
    if (fileName === undefined) continue

    let siblings = roots
    let prefix = ''
    for (const segment of segments) {
      prefix = prefix === '' ? segment : `${prefix}/${segment}`
      let directory = directories.get(prefix)
      if (!directory) {
        directory = { kind: 'directory', path: prefix, name: segment, children: [] }
        directories.set(prefix, directory)
        siblings.push(directory)
      }
      siblings = directory.children
    }

    siblings.push({ kind: 'file', path: change.path, name: fileName, change })
  }

  return roots
}

/**
 * The visible rows for the given collapsed directories.
 *
 * Collapsing hides a directory's children and leaves the directory itself, which
 * is what keeps the row the keyboard is on from disappearing under it.
 */
export function visibleTreeRows(
  nodes: readonly ReviewTreeNode[],
  collapsed: ReadonlySet<string>,
  depth = 0,
): { node: ReviewTreeNode; depth: number }[] {
  const rows: { node: ReviewTreeNode; depth: number }[] = []
  for (const node of nodes) {
    rows.push({ node, depth })
    if (node.kind === 'directory' && !collapsed.has(node.path)) {
      rows.push(...visibleTreeRows(node.children, collapsed, depth + 1))
    }
  }
  return rows
}

/** The paths of every directory in the tree, deepest last. */
export function directoryPaths(nodes: readonly ReviewTreeNode[]): string[] {
  const paths: string[] = []
  for (const node of nodes) {
    if (node.kind !== 'directory') continue
    paths.push(node.path, ...directoryPaths(node.children))
  }
  return paths
}
