import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ReviewPanel from '../src/components/changes/ReviewPanel.vue'

import type { ReviewChange } from '../src/components/changes/review-filters'
import {
  buildReviewTree,
  directoryPaths,
  visibleTreeRows,
} from '../src/components/changes/review-tree'

/**
 * The Review tab file tree, task U5-02 of the implementation plan.
 *
 * Section 4.7 asks for a file tree ordered identically to the diff list and for
 * expand/collapse navigation. The ordering is the whole point of the task, so it
 * is pinned directly: flattening the tree has to give back the list it came from.
 */

function change(path: string): ReviewChange {
  return { path, kind: 'modified', staged: false, unstaged: true, turn: 1 }
}

const changes = [
  change('src/app.ts'),
  change('src/components/ReviewPanel.vue'),
  change('docs/design/spec.md'),
  change('README.md'),
]

describe('buildReviewTree', () => {
  it('nests a path under the directories it names', () => {
    const tree = buildReviewTree(changes)

    const src = tree[0]
    expect(src?.kind).toBe('directory')
    if (src?.kind !== 'directory') throw new Error('expected a directory')
    expect(src.name).toBe('src')
    expect(src.children.map((child) => child.name)).toEqual(['app.ts', 'components'])
  })

  it('reuses one directory node for every change under it', () => {
    const tree = buildReviewTree([
      change('src/a.ts'),
      change('src/b.ts'),
      change('src/nested/c.ts'),
    ])

    expect(tree).toHaveLength(1)
    const src = tree[0]
    if (src?.kind !== 'directory') throw new Error('expected a directory')
    expect(src.children.filter((child) => child.kind === 'file')).toHaveLength(2)
  })

  it('orders the tree the way the diff list is ordered', () => {
    // Section 4.7 requires the two orders to match. A directory therefore sits
    // where its first change sat, and its children keep the order they were
    // given; sorting either by name would break the one thing being compared.
    const tree = buildReviewTree(changes)
    const flat = visibleTreeRows(tree, new Set())
      .filter((row) => row.node.kind === 'file')
      .map((row) => (row.node as { path: string }).path)

    expect(flat).toEqual(changes.map((entry) => entry.path))
  })

  it('places a root-level file after the directories that came before it', () => {
    const tree = buildReviewTree(changes)

    expect(tree.map((node) => node.name)).toEqual(['src', 'docs', 'README.md'])
  })
})

describe('visibleTreeRows', () => {
  it('hides the children of a collapsed directory but keeps the directory', () => {
    const tree = buildReviewTree(changes)

    const rows = visibleTreeRows(tree, new Set(['src']))
    const names = rows.map((row) => row.node.name)

    expect(names).toContain('src')
    expect(names).not.toContain('app.ts')
    expect(names).not.toContain('components')
  })

  it('indents a child one level deeper than its directory', () => {
    const tree = buildReviewTree([change('src/app.ts')])
    const rows = visibleTreeRows(tree, new Set())

    expect(rows.map((row) => row.depth)).toEqual([0, 1])
  })

  it('collapses a nested directory without collapsing its parent', () => {
    const tree = buildReviewTree([change('src/components/ReviewPanel.vue')])

    const rows = visibleTreeRows(tree, new Set(['src/components']))
    const names = rows.map((row) => row.node.name)

    expect(names).toEqual(['src', 'components'])
  })
})

describe('directoryPaths', () => {
  it('names every directory so a caller can collapse them all', () => {
    const tree = buildReviewTree(changes)

    expect(directoryPaths(tree).sort()).toEqual(['docs', 'docs/design', 'src', 'src/components'])
  })
})

describe('ReviewPanel tree', () => {
  const nested = [
    change('src/app.ts'),
    change('src/components/ReviewPanel.vue'),
    change('README.md'),
  ]

  function mountPanel() {
    return mount(ReviewPanel, { props: { changes: nested, branchChanges: [], lastTurn: 1 } })
  }

  it('renders directories and files as tree items', () => {
    const wrapper = mountPanel()

    expect(wrapper.find('[role="tree"]').exists()).toBe(true)
    // src, src/app.ts, src/components, .../ReviewPanel.vue, README.md
    expect(wrapper.findAll('[role="treeitem"]')).toHaveLength(5)
  })

  it('hides the children of a collapsed directory', async () => {
    const wrapper = mountPanel()

    await wrapper.get('[data-review-directory="src"]').trigger('click')

    expect(wrapper.find('[data-review-file="src/app.ts"]').exists()).toBe(false)
    expect(wrapper.get('[data-review-directory="src"]').attributes('aria-expanded')).toBe('false')
    // The directory itself stays, so the row the reader just clicked does not
    // vanish under their pointer or their keyboard focus.
    expect(wrapper.find('[data-review-directory="src"]').exists()).toBe(true)
  })

  it('brings the children back when the directory is expanded again', async () => {
    const wrapper = mountPanel()

    await wrapper.get('[data-review-directory="src"]').trigger('click')
    await wrapper.get('[data-review-directory="src"]').trigger('click')

    expect(wrapper.find('[data-review-file="src/app.ts"]').exists()).toBe(true)
    expect(wrapper.get('[data-review-directory="src"]').attributes('aria-expanded')).toBe('true')
  })

  it('collapses and expands every directory from one control', async () => {
    const wrapper = mountPanel()

    await wrapper.get('[data-review-toggle-all]').trigger('click')
    // Every directory is closed, so the only file left is the one that sits
    // at the root and belongs to no directory to close.
    expect(wrapper.findAll('[data-review-file]').map((row) => row.attributes('data-review-file'))).toEqual([
      'README.md',
    ])

    await wrapper.get('[data-review-toggle-all]').trigger('click')
    expect(wrapper.findAll('[data-review-file]')).toHaveLength(3)
  })

  it('keeps the file key on the full path, not the leaf name', async () => {
    // The change is keyed by its workspace-relative path everywhere else, so a
    // tree that keyed rows by leaf name would break that identity.
    const wrapper = mountPanel()

    const row = wrapper.get('[data-review-file="src/components/ReviewPanel.vue"]')
    expect(row.text()).toContain('ReviewPanel.vue')
  })

  it('reads in the interface font at its normal weight, as the reference does', () => {
    // The explorer used to be set in the monospace stack with a heavier weight
    // on directories, which made a column of names look like a column of
    // headings. The reference's explorer is the interface font at regular
    // weight, and the tree is read rather than compared - the diff beside it is
    // where the monospace belongs.
    const css = readFileSync(
      resolve(process.cwd(), 'src', 'components', 'changes', 'ReviewPanel.vue'),
      'utf8',
    )

    const path = /\.review-panel__path\s*\{[^}]*\}/s.exec(css)?.[0] ?? ''
    expect(path).toContain('font-family: var(--font-body)')
    expect(path).toContain('font-weight: var(--weight-normal)')

    // No row carries an emphasis of its own: the directory is told apart by its
    // chevron, its colour and its indent.
    const directory = /\.review-panel__row--directory\s*\{[^}]*\}/s.exec(css)?.[0] ?? ''
    expect(directory).not.toContain('font-weight')

    // And the same list the explorer draws beside the tree follows it.
    const inspectorCss = readFileSync(
      resolve(process.cwd(), 'src', 'components', 'changes', 'ChangeInspector.vue'),
      'utf8',
    )
    const inspectorPath = /\.change-inspector__path\s*\{[^}]*\}/s.exec(inspectorCss)?.[0] ?? ''
    expect(inspectorPath).toContain('font-family: var(--font-body)')
    expect(inspectorPath).toContain('font-weight: var(--weight-normal)')

    // The guard against over-correcting: the diff's own text stays monospace,
    // because there the columns have to line up.
    const diffCss = readFileSync(
      resolve(process.cwd(), 'src', 'components', 'changes', 'SafeDiffPreview.vue'),
      'utf8',
    )
    const diffText = /\.safe-diff-preview__text\s*\{[^}]*\}/s.exec(diffCss)?.[0] ?? ''
    expect(diffText).toContain('font-family: var(--font-mono)')
  })
})
