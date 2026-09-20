<script setup lang="ts">
/**
 * One tool invocation, as a card.
 *
 * `call_id` is the identity here, not the message text: the same command can be
 * run twice in a run, and a reader investigating a failure needs to point at the
 * exact call. The detail is collapsed by default because a governed run emits
 * far more calls than a reader wants to read at once, and a failed call says so
 * in the summary line rather than waiting to be opened.
 */
import type { ToolTimelineItem } from '@orchester/ereignis'
import { ChevronRight, Terminal } from '@lucide/vue'
import { computed, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    item: ToolTimelineItem
    expandLabel?: string
    collapseLabel?: string
  }>(),
  {
    expandLabel: 'Show the call detail',
    collapseLabel: 'Hide the call detail',
  },
)

const expanded = ref(false)
const stateLabel = computed(() => {
  switch (props.item.state) {
    case 'succeeded':
      return 'done'
    case 'failed':
      return 'failed'
    case 'cancelled':
      return 'cancelled'
    default:
      return 'running'
  }
})
</script>

<template>
  <div
    class="tool-card"
    data-tool-card
    :data-tool-call-id="props.item.callId"
    :data-tool-state="props.item.state"
    :data-tool-name="props.item.name"
  >
    <Terminal class="tool-card__icon" :size="14" aria-hidden="true" />
    <span class="tool-card__name">{{ props.item.name }}</span>
    <span class="tool-card__state">{{ stateLabel }}</span>
    <button
      v-if="props.item.detail"
      class="tool-card__toggle"
      type="button"
      data-tool-toggle
      :aria-expanded="expanded"
      :aria-label="expanded ? collapseLabel : expandLabel"
      @click="expanded = !expanded"
    >
      <ChevronRight class="tool-card__chevron" :size="14" aria-hidden="true" />
    </button>
    <pre v-if="expanded && props.item.detail" class="tool-card__detail" data-tool-detail>{{
      props.item.detail
    }}</pre>
  </div>
</template>

<style scoped>
.tool-card {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
  font-size: var(--text-sm);
}

.tool-card[data-tool-state='failed'] {
  border-color: var(--color-intent-danger-border);
}

.tool-card__icon {
  color: var(--color-text-tertiary);
}

.tool-card__name {
  overflow: hidden;
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tool-card__state {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.tool-card[data-tool-state='failed'] .tool-card__state {
  color: var(--color-intent-danger-text);
}

.tool-card__toggle {
  display: inline-flex;
  min-inline-size: var(--hit-target-min, 32px);
  min-block-size: var(--hit-target-min, 32px);
  align-items: center;
  justify-content: center;
  border: 0;
  background: none;
  color: inherit;
  cursor: pointer;
}

.tool-card__toggle[aria-expanded='true'] .tool-card__chevron {
  transform: rotate(90deg);
}

.tool-card__detail {
  grid-column: 1 / -1;
  margin: 0;
  overflow: auto;
  max-block-size: 16rem;
  padding: var(--space-2);
  border-radius: var(--radius-xs);
  background: var(--color-bg-base);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  white-space: pre-wrap;
}
</style>
