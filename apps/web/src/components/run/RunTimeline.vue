<script setup lang="ts">
import type { RunView, TimelineItem } from '@orchester/ereignis'

defineProps<{
  view: RunView
}>()

function itemText(item: TimelineItem): string {
  switch (item.type) {
    case 'message':
      return item.text
    case 'reasoning':
      return item.text
    case 'tool':
      return `${item.name} (${item.state})`
    case 'file_change':
      return `${item.kind}: ${item.path}`
    case 'todo_list':
      return `${item.items.length} todo items`
    case 'validation':
      return item.validation.summary
    case 'approval':
      return `${item.approvalId} (${item.state})`
    case 'error':
      return `${item.code}: ${item.message}`
    case 'gap':
      return `Missing sequence ${item.missingFrom}-${item.missingTo}`
    default:
      return assertNever(item)
  }
}

function assertNever(value: never): never {
  throw new Error(`Unhandled timeline item: ${String(value)}`)
}
</script>

<template>
  <ol v-if="view.timeline.length > 0" class="run-timeline" data-run-timeline>
    <li
      v-for="item in view.timeline"
      :key="item.key"
      class="run-timeline__item"
      :class="`run-timeline__item--${item.type}`"
      :data-item-type="item.type"
    >
      <span class="run-timeline__sequence">{{ item.sequence }}</span>
      <span class="run-timeline__body">{{ itemText(item) }}</span>
    </li>
  </ol>
</template>

<style scoped>
.run-timeline {
  display: grid;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-4);
  list-style: none;
}

.run-timeline__item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--space-3);
  min-inline-size: 0;
  padding: var(--space-3);
  border-inline-start: 2px solid var(--color-border-strong);
  background: var(--color-bg-surface);
}

.run-timeline__item--gap {
  border-inline-start-color: var(--color-status-warning);
  color: var(--color-status-warning);
}

.run-timeline__sequence {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.run-timeline__body {
  min-inline-size: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
</style>
