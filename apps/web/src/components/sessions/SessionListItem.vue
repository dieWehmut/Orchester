<script setup lang="ts">
import type { SessionSummaryDto } from '@orchester/protokoll'
import { AppBadge, StatusDot } from '@orchester/design'
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
</script>

<template>
  <button
    class="session-list-item"
    :class="{ 'session-list-item--selected': selected }"
    type="button"
    :aria-pressed="selected"
    :data-session-id="session.id"
    @click="$emit('select', session.id)"
  >
    <span class="session-list-item__heading">
      <StatusDot :status="status" :label="session.outcome" :pulse="false" />
      <strong>{{ session.title }}</strong>
      <time :datetime="dateTime.toISOString()">{{ displayTime }}</time>
    </span>
    <span class="session-list-item__metadata">
      <span>{{ session.agent }}</span>
      <span v-if="session.model">{{ session.model }}</span>
      <AppBadge v-if="session.resumable" tone="info">{{ t('sessions.resumable') }}</AppBadge>
    </span>
  </button>
</template>

<style scoped>
.session-list-item {
  display: grid;
  inline-size: 100%;
  gap: var(--space-2);
  padding: var(--space-3);
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

.session-list-item__heading {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-2);
}

.session-list-item__heading strong {
  overflow: hidden;
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-list-item__heading time,
.session-list-item__metadata {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.session-list-item__metadata {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  padding-inline-start: calc(8px + var(--space-2));
}

.session-list-item__metadata > span:not(.app-badge) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
