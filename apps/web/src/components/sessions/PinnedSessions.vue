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
import SessionListItem from './SessionListItem.vue'

const props = defineProps<{
  items: readonly SessionSummaryDto[]
  pinnedIds: readonly string[]
  selectedId: string | null
}>()

defineEmits<{ select: [id: string]; 'toggle-pin': [id: string] }>()

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
  <!--
    Nothing to say when nothing is pinned: the reference's own pinned list is a
    heading and its rows, and the control that adds one says "pin to the top" on
    every session row it sits beside. A sentence here would be a second
    explanation of the same thing, in the place the reference leaves empty.
  -->
  <nav
    v-if="rows.length > 0"
    class="pinned-sessions"
    data-pinned-sessions
    :aria-label="t('sessions.pinned')"
  >
    <SessionListItem
      v-for="row in rows"
      :key="row.id"
      :session="row"
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
  gap: var(--space-1);
}
</style>