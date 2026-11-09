<script setup lang="ts">
/**
 * The reader's own list, at the top of the rail.
 *
 * The reference's sidebar opens with a pinned list. A pin is the reader's
 * ordering rather than a fact about the run, so the ids live in the browser; the
 * rows are drawn from the page the rail has loaded, which means a pin whose run
 * is not on this page is not shown here - the rail holds a cursor, not the whole
 * history, and inventing a row for something it cannot name would be worse.
 */
import type { SessionSummaryDto } from '@orchester/protokoll'
import { computed } from 'vue'

import { useI18n } from '../../i18n'
import type { SessionAction, SessionMetadata, SessionDestination } from './session-actions'
import SessionListItem from './SessionListItem.vue'

const props = defineProps<{
  items: readonly SessionSummaryDto[]
  pinnedIds: readonly string[]
  metadata?: Record<string, SessionMetadata>
  projects?: readonly SessionDestination[]
  sections?: readonly SessionDestination[]
  selectedId: string | null
}>()

defineEmits<{ select: [id: string]; 'toggle-pin': [id: string]; action: [action: SessionAction] }>()

const { t } = useI18n()

/** In the order the reader pinned them, which is the order the store keeps. */
const rows = computed(() =>
  props.pinnedIds.flatMap((id) => {
    const found = props.items.find((item) => item.id === id)
    return found === undefined ? [] : [found]
  }),
)
</script>

<template>
  <p v-if="rows.length === 0" class="pinned-sessions__empty" data-pinned-empty>
    {{ t('sessions.pinnedEmpty') }}
  </p>
  <nav v-else class="pinned-sessions" data-pinned-sessions :aria-label="t('sessions.pinned')">
    <SessionListItem
      v-for="row in rows"
      :key="row.id"
      :session="row"
          :unread="metadata?.[row.id]?.unread"
          :archived="metadata?.[row.id]?.archived"
          :project-id="metadata?.[row.id]?.project_id"
          :section-id="metadata?.[row.id]?.section_id"
          :projects="projects" :sections="sections"
          @action="$emit('action', $event)"
      :selected="selectedId === row.id"
      pinned
      @select="$emit('select', $event)"
      @toggle-pin="$emit('toggle-pin', $event)"
    />
  </nav>
</template>

<style scoped>
.pinned-sessions {
  display: grid;
  gap: 1px;
}

/* The reference's rail states a group heading and then its rows, with no
   sentence explaining what an empty group would hold. The note stays, because
   a reader who pinned something and then cannot find it needs to know where it
   went, but it drops to the quietest type in the column and loses its
   paragraph padding so the column keeps its rhythm. */
.pinned-sessions__empty {
  margin: 0;
  padding: var(--space-1) var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  line-height: var(--leading-snug);
}
</style>