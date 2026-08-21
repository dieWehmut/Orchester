<script setup lang="ts">
import { FilePenLine, FilePlus2, FileX2 } from '@lucide/vue'
import { AppBadge, EmptyState } from '@orchester/design'
import { computed } from 'vue'

import type { ChangeSummary } from './change-summary'
import { useI18n } from '../../i18n'

const props = withDefaults(
  defineProps<{
    changes: readonly ChangeSummary[]
    selectedPath?: string | null
  }>(),
  { selectedPath: null },
)

const { t } = useI18n()

defineEmits<{
  select: [path: string]
}>()

const countLabel = computed(
  () =>
    `${props.changes.length} ${props.changes.length === 1 ? t('inspector.file') : t('inspector.files')}`,
)

function kindLabel(kind: ChangeSummary['kind']): string {
  switch (kind) {
    case 'add':
      return t('inspector.added')
    case 'update':
      return t('inspector.modified')
    case 'delete':
      return t('inspector.deleted')
  }
}

function badgeTone(kind: ChangeSummary['kind']): 'success' | 'info' | 'error' {
  switch (kind) {
    case 'add':
      return 'success'
    case 'update':
      return 'info'
    case 'delete':
      return 'error'
  }
}
</script>

<template>
  <section class="change-inspector" :aria-label="countLabel">
    <header v-if="props.changes.length > 0" class="change-inspector__header">
      <strong>{{ countLabel }}</strong>
      <span>{{ t('inspector.observedEvents') }}</span>
    </header>

    <div v-if="props.changes.length === 0" data-change-empty>
      <EmptyState
        :title="t('inspector.noFileChanges')"
        :description="t('inspector.noFileChangesDescription')"
      />
    </div>

    <div v-else class="change-inspector__list" role="list">
      <button
        v-for="change in props.changes"
        :key="change.path"
        class="change-inspector__row"
        :class="{ 'change-inspector__row--selected': props.selectedPath === change.path }"
        type="button"
        role="listitem"
        :aria-pressed="props.selectedPath === change.path"
        :aria-label="`${change.path}, ${kindLabel(change.kind)}`"
        :data-change-path="change.path"
        :data-change-kind="change.kind"
        @click="$emit('select', change.path)"
      >
        <span class="change-inspector__icon" aria-hidden="true">
          <FilePlus2 v-if="change.kind === 'add'" :size="17" :stroke-width="1.8" />
          <FilePenLine v-else-if="change.kind === 'update'" :size="17" :stroke-width="1.8" />
          <FileX2 v-else :size="17" :stroke-width="1.8" />
        </span>
        <span class="change-inspector__body">
          <span class="change-inspector__path">{{ change.path }}</span>
          <span class="change-inspector__meta">
            <AppBadge :tone="badgeTone(change.kind)">{{ kindLabel(change.kind) }}</AppBadge>
            <span>{{ change.eventCount }} {{ change.eventCount === 1 ? t('inspector.event') : t('inspector.events') }}</span>
            <span class="change-inspector__sequence">{{ t('inspector.sequence') }}{{ change.latestSequence }}</span>
          </span>
        </span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.change-inspector {
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
  gap: var(--space-2);
}

.change-inspector__header {
  display: flex;
  min-block-size: 2rem;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding-inline: var(--space-1);
}

.change-inspector__header strong {
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
}

.change-inspector__header span {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.change-inspector__list {
  display: grid;
  gap: var(--space-1);
}

.change-inspector__row {
  display: grid;
  grid-template-columns: 1.75rem minmax(0, 1fr);
  min-block-size: 4rem;
  align-items: center;
  inline-size: 100%;
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

.change-inspector__row:hover,
.change-inspector__row:focus-visible,
.change-inspector__row--selected {
  border-color: var(--color-border-base);
  background: var(--color-bg-element);
  outline: none;
}

.change-inspector__row--selected {
  border-color: var(--color-accent-border);
}

.change-inspector__icon {
  display: grid;
  inline-size: 1.75rem;
  block-size: 1.75rem;
  place-items: center;
  color: var(--color-text-secondary);
}

.change-inspector__body,
.change-inspector__meta {
  display: flex;
  min-inline-size: 0;
}

.change-inspector__body {
  flex-direction: column;
  gap: var(--space-1);
}

.change-inspector__path {
  overflow: hidden;
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.change-inspector__meta {
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.change-inspector__sequence {
  margin-inline-start: auto;
  font-family: var(--font-mono);
}
</style>
