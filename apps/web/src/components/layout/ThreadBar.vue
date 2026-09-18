<script setup lang="ts">
import { ChevronDown, Ellipsis, PanelRightClose, PanelRightOpen, Share, Sparkles } from '@lucide/vue'

import { IconButton } from '@orchester/design'

withDefaults(
  defineProps<{
    title: string
    shareLabel: string
    moreLabel: string
    panelLabel: string
    panelOpen?: boolean
    kind?: 'thread' | 'draft'
    /** Who is answering. Falls back to the product name when none is chosen. */
    agentName?: string | null
    /** The agent's state, already translated: the bar does not know the states. */
    agentStatus?: string | null
    /** Whether that state should read as reachable. */
    agentOnline?: boolean
    /** The label on the share action; hidden when the surface passes none. */
    shareText?: string | null
  }>(),
  {
    panelOpen: true,
    kind: 'thread',
    agentName: null,
    agentStatus: null,
    agentOnline: false,
    shareText: null,
  },
)

defineEmits<{
  share: []
  more: []
  togglePanel: []
}>()
</script>

<template>
  <header class="thread-bar" data-thread-bar :data-agent-online="agentOnline ? 'true' : 'false'">
    <span class="thread-bar__icon" aria-hidden="true">
      <Sparkles v-if="kind === 'thread'" :size="16" />
      <PanelRightOpen v-else :size="16" />
    </span>

    <span class="thread-bar__identity">
      <span class="thread-bar__heading">
        <h1 class="thread-bar__title" data-thread-title>{{ title }}</h1>
        <ChevronDown class="thread-bar__chevron" :size="14" aria-hidden="true" />
      </span>
      <span v-if="agentName" class="thread-bar__agent" data-thread-agent>
        <span
          class="thread-bar__status-dot"
          :class="{ 'thread-bar__status-dot--online': agentOnline }"
          aria-hidden="true"
        />
        <span data-thread-status>{{ agentName }}<template v-if="agentStatus"> · {{ agentStatus }}</template></span>
      </span>
    </span>

    <div class="thread-bar__actions">
      <button
        class="thread-bar__share"
        type="button"
        :aria-label="shareLabel"
        data-thread-action="share"
        @click="$emit('share')"
      >
        <Share :size="15" aria-hidden="true" />
        <span v-if="shareText">{{ shareText }}</span>
      </button>
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

.thread-bar__identity {
  display: grid;
  min-inline-size: 0;
  flex: 1;
  gap: 1px;
  line-height: var(--leading-tight);
}

.thread-bar__heading {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-1);
}

.thread-bar__title {
  min-inline-size: 0;
  overflow: hidden;
  margin: 0;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thread-bar__chevron {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
}

/* The identity line is what the reference puts above the transcript: who is
   answering, and whether they are reachable. The dot is the whole state
   signal; the text beside it spells it out for anyone who cannot see it. */
.thread-bar__agent {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.thread-bar__agent > span:last-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thread-bar__status-dot {
  inline-size: 0.4rem;
  block-size: 0.4rem;
  flex: 0 0 auto;
  border-radius: var(--radius-full);
  background: var(--color-text-tertiary);
}

.thread-bar__status-dot--online {
  background: var(--color-status-success);
}

.thread-bar__share {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}

.thread-bar__share:hover {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.thread-bar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}
</style>