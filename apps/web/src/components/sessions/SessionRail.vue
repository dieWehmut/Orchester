<script setup lang="ts">
import type { SessionSummaryDto } from '@orchester/protokoll'
import { AppButton, EmptyState, InlineAlert, SkeletonBlock } from '@orchester/design'
import { computed } from 'vue'

import { useI18n } from '../../i18n'
import type { SessionsStatus } from '../../stores/sessions'
import SessionListItem from './SessionListItem.vue'

const props = withDefaults(
  defineProps<{
  status: SessionsStatus
  items: SessionSummaryDto[]
  selectedId: string | null
  nextCursor: string | null
  error: { message: string; retryable: boolean } | null
  /**
   * The rail header's search text.
   *
   * The runtime's `/sessions` route pages by cursor and takes no query, so this
   * narrows the page the rail already holds rather than pretending to search
   * the whole history. That is why the empty state below distinguishes "no
   * sessions" from "nothing matched": a filter that reached the runtime would
   * have no need for the distinction.
   */
  query?: string
  /**
   * The runs the reader pinned, which the pinned list above already shows.
   *
   * A run appears once: what is recent is not a second copy of what the reader
   * chose to keep.
   */
  pinnedIds?: readonly string[]
  }>(),
  { query: '', pinnedIds: () => [] },
)

defineEmits<{
  select: [id: string]
  refresh: []
  loadMore: []
  togglePin: [id: string]
}>()

const { t } = useI18n()

/** Whether one session answers the filter, over the fields a row shows. */
function admits(item: SessionSummaryDto, needle: string): boolean {
  return [item.title, item.agent, item.model ?? ''].some((field) =>
    field.toLowerCase().includes(needle),
  )
}

/**
 * The sessions the filter admits, less the pinned ones.
 *
 * Matching is case-insensitive and runs over the fields a reader can see in a
 * row - the title, the agent and the model - because a filter that matched
 * something invisible to the reader would return rows with no visible reason
 * to be there.
 */
const matches = computed<SessionSummaryDto[]>(() => {
  const needle = props.query.trim().toLowerCase()
  const admitted =
    needle.length === 0 ? props.items : props.items.filter((item) => admits(item, needle))
  const pinned = new Set(props.pinnedIds)
  return admitted.filter((item) => !pinned.has(item.id))
})

/**
 * Whether the filter admitted nothing at all.
 *
 * Asked of the whole page rather than of this list: when the only match is a
 * pinned run, the pinned list above is showing it, and a "nothing matched"
 * message here would contradict the row the reader can see.
 */
const filteredEmpty = computed(() => {
  const needle = props.query.trim().toLowerCase()
  if (needle.length === 0 || props.items.length === 0) return false
  return !props.items.some((item) => admits(item, needle))
})
</script>

<template>
  <div
    class="session-rail"
    data-session-rail
    :aria-busy="status === 'loading' || status === 'refreshing'"
  >
    <div v-if="status === 'loading' && items.length === 0" class="session-rail__loading" role="status">
      <SkeletonBlock :lines="6" height="3.25rem" />
    </div>

    <InlineAlert
      v-else-if="status === 'error' && items.length === 0"
      tone="error"
      :title="t('sessions.loadError')"
    >
      <p>{{ error?.message }}</p>
      <AppButton v-if="error?.retryable" size="sm" variant="secondary" @click="$emit('refresh')">
        {{ t('sessions.retry') }}
      </AppButton>
    </InlineAlert>

    <p
      v-else-if="filteredEmpty"
      class="session-rail__filter-empty"
      data-session-filter-empty
      role="status"
    >
      {{ t('sessions.filterEmpty') }}
    </p>

    <EmptyState
      v-else-if="status !== 'idle' && items.length === 0"
      :title="t('sessions.empty')"
      :description="t('sessions.emptyDescription')"
    />

    <template v-else>
      <InlineAlert v-if="status === 'error'" tone="warning" :title="t('sessions.loadError')">
        {{ error?.message }}
      </InlineAlert>
      <nav class="session-rail__list" :aria-label="t('sessions.title')">
        <SessionListItem
          v-for="session in matches"
          :key="session.id"
          :session="session"
          :selected="selectedId === session.id"
          @select="$emit('select', $event)"
          @toggle-pin="$emit('togglePin', $event)"
        />
      </nav>
      <AppButton
        v-if="nextCursor"
        class="session-rail__more"
        variant="ghost"
        size="sm"
        block
        :busy="status === 'loading_more'"
        @click="$emit('loadMore')"
      >
        {{ t('sessions.loadMore') }}
      </AppButton>
    </template>
  </div>
</template>

<style scoped>
.session-rail {
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
  gap: var(--space-3);
}

.session-rail__loading {
  padding-block: var(--space-2);
}

.session-rail__list {
  display: grid;
  gap: var(--space-1);
}

.session-rail__more {
  margin-block-start: auto;
}

/* The filter's empty state is a sentence rather than the list's full empty
   state: the runtime did send sessions, and the reader only has to widen the
   filter, which the empty state's action button could not offer. */
.session-rail__filter-empty {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}

.session-rail :deep(.inline-alert p) {
  margin: 0 0 var(--space-2);
}
</style>
