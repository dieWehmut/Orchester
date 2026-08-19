<script setup lang="ts">
import type { SessionSummaryDto } from '@orchester/protokoll'
import { AppButton, EmptyState, InlineAlert, SkeletonBlock } from '@orchester/design'

import { useI18n } from '../../i18n'
import type { SessionsStatus } from '../../stores/sessions'
import SessionListItem from './SessionListItem.vue'

defineProps<{
  status: SessionsStatus
  items: SessionSummaryDto[]
  selectedId: string | null
  nextCursor: string | null
  error: { message: string; retryable: boolean } | null
}>()

defineEmits<{
  select: [id: string]
  refresh: []
  loadMore: []
  newSession: []
}>()

const { t } = useI18n()
</script>

<template>
  <div class="session-rail" :aria-busy="status === 'loading' || status === 'refreshing'">
    <header class="session-rail__header">
      <h2>{{ t('sessions.title') }}</h2>
      <AppButton size="sm" @click="$emit('newSession')">{{ t('sessions.new') }}</AppButton>
    </header>

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
          v-for="session in items"
          :key="session.id"
          :session="session"
          :selected="selectedId === session.id"
          @select="$emit('select', $event)"
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
  padding: var(--space-3);
}

.session-rail__header {
  display: flex;
  min-block-size: var(--control-height-lg);
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.session-rail__header h2 {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  letter-spacing: 0.04em;
  text-transform: uppercase;
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

.session-rail :deep(.inline-alert p) {
  margin: 0 0 var(--space-2);
}
</style>
