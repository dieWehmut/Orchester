<script setup lang="ts">
import type { SessionSummaryDto } from '@orchester/protokoll'
import { StatusDot, VisuallyHidden } from '@orchester/design'
import { computed } from 'vue'

import { useI18n } from '../../i18n'

const props = defineProps<{
  session: SessionSummaryDto
  selected: boolean
}>()

defineEmits<{ select: [id: string] }>()

const { t } = useI18n()
const dateTime = computed(() => new Date(props.session.recorded_at_unix * 1000))
const displayTime = computed(() =>
  new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(
    dateTime.value,
  ),
)
const status = computed(() => {
  if (props.session.outcome === 'success') return 'success' as const
  if (props.session.outcome === 'failed') return 'error' as const
  return 'idle' as const
})

/**
 * What the row is about, rather than what it is called.
 *
 * The reference's list is one line per entry, and a run's agent and model are
 * the pane's business - the pane states them in full. They stay one hover, or
 * one swipe in a screen reader, away rather than on the row. The outcome is not
 * in this list because the dot beside the title already carries it as its own
 * accessible name.
 */
const details = computed(() =>
  [props.session.agent, props.session.model ?? '', props.session.resumable ? t('sessions.resumable') : '']
    .filter((part) => part.length > 0)
    .join(' · '),
)
</script>

<template>
  <button
    class="session-list-item"
    :class="{ 'session-list-item--selected': selected }"
    type="button"
    :aria-pressed="selected"
    :data-session-id="session.id"
    :title="`${session.title} · ${details}`"
    @click="$emit('select', session.id)"
  >
    <StatusDot :status="status" :label="session.outcome" :pulse="false" />
    <strong class="session-list-item__title">{{ session.title }}</strong>
    <time class="session-list-item__time" :datetime="dateTime.toISOString()">{{ displayTime }}</time>
    <VisuallyHidden data-session-details>{{ details }}</VisuallyHidden>
  </button>
</template>

<style scoped>
.session-list-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  min-block-size: var(--hit-target-min, 32px);
  inline-size: 100%;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-primary);
  font: inherit;
  text-align: start;
  cursor: pointer;
}

.session-list-item:hover,
.session-list-item--selected {
  background: var(--color-bg-element);
}

.session-list-item--selected {
  border-color: var(--color-accent-border);
  box-shadow: inset 2px 0 0 var(--color-accent);
}

.session-list-item__title {
  overflow: hidden;
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-list-item__time {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  white-space: nowrap;
}
</style>
