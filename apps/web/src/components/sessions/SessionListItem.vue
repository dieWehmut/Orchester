<script setup lang="ts">
import type { SessionSummaryDto } from '@orchester/protokoll'
import { IconButton, VisuallyHidden } from '@orchester/design'
import { CircleAlert, MessageCircle, Pin } from '@lucide/vue'
import { computed } from 'vue'

import { useI18n } from '../../i18n'

const props = withDefaults(
  defineProps<{
    session: SessionSummaryDto
    selected: boolean
    pinned?: boolean
  }>(),
  { pinned: false },
)

defineEmits<{ select: [id: string]; 'toggle-pin': [id: string] }>()

const { t } = useI18n()
const dateTime = computed(() => new Date(props.session.recorded_at_unix * 1000))
const displayTime = computed(() =>
  new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(
    dateTime.value,
  ),
)

/**
 * What the row is about, rather than what it is called.
 *
 * The reference's list is one line per entry, and a run's agent and model are
 * the pane's business - the pane states them in full. They stay one hover, or
 * one swipe in a screen reader, away rather than on the row. Dates and completed
 * outcomes stay in accessible detail; only failures need a visible indicator.
 */
const details = computed(() =>
  [props.session.agent, props.session.model ?? '', props.session.resumable ? t('sessions.resumable') : '']
    .filter((part) => part.length > 0)
    .join(' · '),
)
</script>

<template>
  <div
    class="session-list-item"
    :class="{
      'session-list-item--selected': selected,
      'session-list-item--pinned': pinned,
    }"
    :data-session-row="session.id"
  >
    <button
      class="session-list-item__open"
      type="button"
      :aria-pressed="selected"
      :data-session-id="session.id"
      :title="`${session.title} · ${details} · ${displayTime}`"
      @click="$emit('select', session.id)"
    >
      <MessageCircle v-if="pinned" :size="18" aria-hidden="true" />
      <strong class="session-list-item__title">{{ session.title }}</strong>
      <CircleAlert v-if="session.outcome === 'failed'" :size="14" class="session-list-item__error" :aria-label="session.outcome" />
      <VisuallyHidden><time :datetime="dateTime.toISOString()">{{ displayTime }}</time><span>{{ session.outcome }}</span></VisuallyHidden>
      <VisuallyHidden data-session-details>{{ details }}</VisuallyHidden>
    </button>
    <!-- A control beside the row rather than inside it: a button within a button
         is not something a browser will let the reader press. It is drawn on
         hover or focus and never removed from the tree. -->
    <IconButton
      class="session-list-item__pin"
      :label="pinned ? t('sessions.unpinAction') : t('sessions.pinAction')"
      :active="pinned"
      :data-session-pin="session.id"
      @click="$emit('toggle-pin', session.id)"
    >
      <Pin :size="14" aria-hidden="true" />
    </IconButton>
  </div>
</template>

<style scoped>
/* The row is the container and the open control fills it: the pin sits beside
   the row rather than inside it, because the row is a button. */
.session-list-item {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-block-size: var(--hit-target-min, 32px);
  inline-size: 100%;
  align-items: center;
  border: 1px solid transparent;
  border-radius: 10px;
  transition: background var(--transition-fast) var(--ease-out);
}

.session-list-item__open {
  display: flex;
  min-block-size: 36px;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--color-text-primary);
  font: inherit;
  text-align: start;
  cursor: pointer;
}

.session-list-item:hover,
.session-list-item--selected {
  background: var(--workspace-hover, var(--color-bg-element));
}

.session-list-item--selected {
  background: var(--color-bg-element);
}

/* Drawn on hover or focus, never removed from the tree; a pinned row shows it
   because that is the state it reports. */
.session-list-item__pin {
  position: absolute;
  inset-inline-end: 0;
  justify-self: end;
  margin-inline-end: var(--space-1);
  opacity: 0;
  transition: opacity var(--transition-fast) var(--ease-out);
}

.session-list-item:hover .session-list-item__pin,
.session-list-item:focus-within .session-list-item__pin {
  opacity: 1;
}

.session-list-item--pinned .session-list-item__pin {
  color: var(--color-accent);
}

.session-list-item__title {
  flex: 1;
  min-inline-size: 0;
  overflow: hidden;
  font-size: 14px;
  font-weight: var(--weight-normal);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-list-item__open > svg { flex-shrink: 0; }
.session-list-item__error { color: var(--color-status-warning); }
.session-list-item:hover .session-list-item__open, .session-list-item:focus-within .session-list-item__open { padding-inline-end: 34px; }
</style>
