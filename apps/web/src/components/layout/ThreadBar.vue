<script setup lang="ts">
import { Ellipsis, MessageSquareText, PanelRightClose, PanelRightOpen, Share } from '@lucide/vue'

import { IconButton } from '@orchester/design'

withDefaults(
  defineProps<{
    title: string
    shareLabel: string
    moreLabel: string
    panelLabel: string
    panelOpen?: boolean
    kind?: 'thread' | 'draft'
  }>(),
  { panelOpen: true, kind: 'thread' },
)

defineEmits<{
  share: []
  more: []
  togglePanel: []
}>()
</script>

<template>
  <header class="thread-bar" data-thread-bar>
    <span class="thread-bar__icon" aria-hidden="true">
      <MessageSquareText v-if="kind === 'thread'" :size="16" />
      <PanelRightOpen v-else :size="16" />
    </span>
    <h1 class="thread-bar__title" data-thread-title>{{ title }}</h1>

    <div class="thread-bar__actions">
      <IconButton :label="shareLabel" data-thread-action="share" @click="$emit('share')">
        <Share :size="15" aria-hidden="true" />
      </IconButton>
      <IconButton :label="moreLabel" data-thread-action="more" @click="$emit('more')">
        <Ellipsis :size="16" aria-hidden="true" />
      </IconButton>
      <IconButton
        :label="panelLabel"
        :active="panelOpen"
        data-thread-action="panel"
        @click="$emit('togglePanel')"
      >
        <PanelRightClose v-if="panelOpen" :size="16" aria-hidden="true" />
        <PanelRightOpen v-else :size="16" aria-hidden="true" />
      </IconButton>
    </div>
  </header>
</template>

<style scoped>
.thread-bar {
  display: flex;
  min-block-size: var(--header-height);
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-4);
  border-block-end: 1px solid var(--color-border-base);
  background: var(--color-bg-base);
}

.thread-bar__icon {
  display: grid;
  place-items: center;
  color: var(--color-text-tertiary);
}

.thread-bar__title {
  min-inline-size: 0;
  flex: 1;
  overflow: hidden;
  margin: 0;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thread-bar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}
</style>