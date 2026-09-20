<script setup lang="ts">
import { AppBadge, AppSegmentedControl, EmptyState, type AppSegmentOption } from '@orchester/design'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'
import { filterChanges, type ReviewChange, type ReviewFilter } from './review-filters'

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

    <div v-if="visibleChanges.length === 0" data-review-empty>
      <EmptyState
        :title="t('inspector.noFileChanges')"
        :description="t('inspector.noFileChangesDescription')"
      />
    </div>

    <div v-else class="review-panel__list" role="list">
      <button
        v-for="change in visibleChanges"
        :key="change.path"
        class="review-panel__row"
        type="button"
        role="listitem"
        :aria-label="`${change.path}, ${kindLabel(change.kind)}`"
        :data-review-file="change.path"
        :data-review-kind="change.kind"
        :data-review-staged="change.staged"
        :data-review-unstaged="change.unstaged"
      >
        <span class="review-panel__path">{{ change.path }}</span>
        <span class="review-panel__meta">
          <AppBadge :tone="badgeTone(change.kind)">{{ kindLabel(change.kind) }}</AppBadge>
        </span>
      </button>
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
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.review-panel__meta {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
}
</style>
