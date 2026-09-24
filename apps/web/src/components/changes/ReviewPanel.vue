<script setup lang="ts">
import { ChevronDown, ChevronRight, ChevronsDownUp, ChevronsUpDown } from '@lucide/vue'
import {
  AppBadge,
  AppSegmentedControl,
  EmptyState,
  IconButton,
  type AppSegmentOption,
} from '@orchester/design'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'
import { filterChanges, type ReviewChange, type ReviewFilter } from './review-filters'
import { buildReviewTree, directoryPaths, visibleTreeRows } from './review-tree'

/**
 * The Review tab, section 4.7 of the design spec.
 *
 * The five filters are the reader's question about the change set - what is
 * staged, what is not, what this branch did, what this turn did - so the filter
 * is state here and the rules are in `review-filters.ts`, where they are
 * decidable without a rendered tree.
 */

const props = withDefaults(
  defineProps<{
    changes: readonly ReviewChange[]
    branchChanges?: readonly string[]
    lastTurn?: number | null
  }>(),
  { branchChanges: () => [], lastTurn: null },
)

const { t } = useI18n()
const activeFilter = ref<ReviewFilter>('all')

const filterOptions = computed<AppSegmentOption[]>(() => [
  { id: 'all', label: t('inspector.filter.all') },
  { id: 'staged', label: t('inspector.filter.staged') },
  { id: 'unstaged', label: t('inspector.filter.unstaged') },
  { id: 'branch', label: t('inspector.filter.branch') },
  { id: 'last-turn', label: t('inspector.filter.lastTurn') },
])

const visibleChanges = computed(() =>
  filterChanges(props.changes, activeFilter.value, {
    branchChanges: props.branchChanges,
    lastTurn: props.lastTurn,
  }),
)

/**
* The tree, section 4.7: ordered identically to the diff list.
*
* Built from the filtered changes rather than the whole set, so a filter that
* admits nothing shows an empty tree instead of a tree of things it excluded.
*/
const tree = computed(() => buildReviewTree(visibleChanges.value))
const collapsed = ref<ReadonlySet<string>>(new Set())
const rows = computed(() => visibleTreeRows(tree.value, collapsed.value))
const allCollapsed = computed(() =>
  collapsed.value.size > 0 && collapsed.value.size >= directoryPaths(tree.value).length,
)

function toggleDirectory(path: string): void {
  const next = new Set(collapsed.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  collapsed.value = next
}

/**
* Collapse or expand every directory at once.
*
* Section 4.7 asks for expand/collapse navigation; one control for the whole
* tree saves the reader from closing a deep tree one directory at a time.
*/
function toggleAll(): void {
  collapsed.value = allCollapsed.value ? new Set() : new Set(directoryPaths(tree.value))
}

function kindLabel(kind: ReviewChange['kind']): string {
  switch (kind) {
    case 'added':
    case 'untracked':
      return t('inspector.added')
    case 'deleted':
      return t('inspector.deleted')
    case 'modified':
      return t('inspector.modified')
  }
}

function badgeTone(kind: ReviewChange['kind']): 'success' | 'info' | 'error' | 'neutral' {
  switch (kind) {
    case 'added':
      return 'success'
    case 'deleted':
      return 'error'
    case 'untracked':
      return 'neutral'
    case 'modified':
      return 'info'
  }
}

function selectFilter(id: string): void {
  activeFilter.value = id as ReviewFilter
}
</script>

<template>
  <section
    class="review-panel"
    :aria-label="t('inspector.review')"
    data-review-panel
    :data-review-filter="activeFilter"
  >
    <AppSegmentedControl
      :model-value="activeFilter"
      :options="filterOptions"
      :ariaLabel="t('inspector.filterLabel')"
      data-review-filters
      @update:model-value="selectFilter"
    />
    <IconButton
      v-if="visibleChanges.length > 0"
      :label="allCollapsed ? t('inspector.expandAll') : t('inspector.collapseAll')"
      data-review-toggle-all
      @click="toggleAll"
    >
      <ChevronsDownUp v-if="allCollapsed" :size="16" :stroke-width="1.8" />
      <ChevronsUpDown v-else :size="16" :stroke-width="1.8" />
    </IconButton>

    <div v-if="visibleChanges.length === 0" data-review-empty>
      <EmptyState
        :title="t('inspector.noFileChanges')"
        :description="t('inspector.noFileChangesDescription')"
      />
    </div>

    <div v-else class="review-panel__list" role="tree" :aria-label="t('inspector.review')">
      <template v-for="row in rows" :key="row.node.kind === 'file' ? row.node.path : row.node.path + '/'">
        <button
          v-if="row.node.kind === 'directory'"
          class="review-panel__row review-panel__row--directory"
          type="button"
          role="treeitem"
          :aria-expanded="!collapsed.has(row.node.path)"
          :style="{ paddingInlineStart: `calc(var(--space-2) + ${row.depth} * var(--space-3))` }"
          :data-review-directory="row.node.path"
          @click="toggleDirectory(row.node.path)"
        >
          <span class="review-panel__chevron" aria-hidden="true">
            <ChevronRight v-if="collapsed.has(row.node.path)" :size="14" :stroke-width="1.8" />
            <ChevronDown v-else :size="14" :stroke-width="1.8" />
          </span>
          <span class="review-panel__path">{{ row.node.name }}</span>
        </button>
        <button
          v-else
          class="review-panel__row"
          type="button"
          role="treeitem"
          :aria-label="`${row.node.path}, ${kindLabel(row.node.change.kind)}`"
          :style="{ paddingInlineStart: `calc(var(--space-2) + ${row.depth} * var(--space-3))` }"
          :data-review-file="row.node.path"
          :data-review-kind="row.node.change.kind"
          :data-review-staged="row.node.change.staged"
          :data-review-unstaged="row.node.change.unstaged"
        >
          <span class="review-panel__path">{{ row.node.name }}</span>
          <span class="review-panel__meta">
            <AppBadge :tone="badgeTone(row.node.change.kind)">{{ kindLabel(row.node.change.kind) }}</AppBadge>
          </span>
        </button>
      </template>
    </div>
  </section>
</template>

<style scoped>
.review-panel {
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
  gap: var(--space-2);
}

.review-panel__list {
  display: grid;
  gap: var(--space-1);
}

.review-panel__row {
  display: flex;
  min-block-size: max(2rem, var(--hit-target-min, 32px));
  align-items: center;
  inline-size: 100%;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-2);
  /* The tree sets the inline start from the row depth, so the base padding
     stays symmetric and only the indent moves. */
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
  cursor: pointer;
}

.review-panel__row:hover {
  border-color: var(--color-border-base);
  background: var(--color-bg-element);
}

.review-panel__row:focus-visible {
  border-color: var(--color-border-base);
  background: var(--color-bg-element);
  outline: 2px solid var(--color-border-focus);
  outline-offset: -2px;
}

.review-panel__path {
  overflow: hidden;
  color: var(--color-text-primary);
  /* The explorer reads in the interface's own font, as the reference's does.
     A monospace stack is for the diff beside this tree, where the columns have
     to line up; here it only made every row look heavier than it is. */
  font-family: var(--font-body);
  font-size: var(--text-sm);
  font-weight: var(--weight-normal);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.review-panel__meta {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
}

.review-panel__row--directory {
  justify-content: flex-start;
  color: var(--color-text-secondary);
}

.review-panel__chevron {
  display: grid;
  inline-size: 1rem;
  place-items: center;
  color: var(--color-text-tertiary);
}
</style>
