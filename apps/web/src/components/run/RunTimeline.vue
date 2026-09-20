<script setup lang="ts">
import type { RunView, TimelineItem } from '@orchester/ereignis'
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import ReasoningDisclosure from './ReasoningDisclosure.vue'
import ToolCallCard from './ToolCallCard.vue'
import { virtualWindow } from './virtual-window'

const props = withDefaults(
  defineProps<{
    view: RunView
    /** The transcript scroller's offset, measured by the panel that owns it. */
    scrollTop?: number
    /** The transcript scroller's viewport, measured by the panel that owns it. */
    viewportHeight?: number
  }>(),
  { scrollTop: 0, viewportHeight: 0 },
)

const list = ref<HTMLElement | null>(null)
const heights = ref<(number | undefined)[]>([])

/** Rows mounted beyond the viewport, so a flick of the wheel lands on real rows. */
const OVERSCAN = 2

/**
 * The mounted rows. Until every row has been measured the window is all of
 * them, so the transcript renders whole on first paint and inside an
 * environment that has no layout to measure.
 */
const windowRange = computed(() =>
  virtualWindow({
    heights: props.view.timeline.map((_, index) => heights.value[index]),
    scrollTop: props.scrollTop,
    viewportHeight: props.viewportHeight,
    overscan: OVERSCAN,
  }),
)

const mounted = computed(() =>
  props.view.timeline.slice(windowRange.value.start, windowRange.value.end),
)

function measure(): void {
  const element = list.value
  if (!element) return
  const rows = Array.from(element.querySelectorAll<HTMLElement>('[data-virtualized-turn]'))
  const measured = [...heights.value]
  for (const row of rows) {
    const index = Number(row.dataset.virtualizedTurn)
    const box = row.getBoundingClientRect().height || row.offsetHeight
    // A zero box is a row that has not been laid out, not a row of no height;
    // recording it as zero would virtualise the transcript away on the next
    // scroll.
    measured[index] = box > 0 ? box : undefined
  }
  heights.value = measured
}

onMounted(async () => {
  await nextTick()
  measure()
})

watch(
  () => props.view.timeline.length,
  async () => {
    await nextTick()
    measure()
  },
)

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
  <ol
    v-if="view.timeline.length > 0"
    ref="list"
    class="run-timeline"
    data-run-timeline
    :data-virtual-window="`${windowRange.start}-${windowRange.end}`"
  >
    <li
      v-if="windowRange.offsetTop > 0"
      class="run-timeline__spacer"
      data-virtual-spacer="top"
      aria-hidden="true"
      :style="{ blockSize: windowRange.offsetTop + 'px' }"
    />
    <li
      v-for="(item, index) in mounted"
      :key="item.key"
      class="run-timeline__item"
      :class="`run-timeline__item--${item.type}`"
      :data-item-type="item.type"
      :data-virtualized-turn="windowRange.start + index"
    >
      <template v-if="item.type === 'tool'">
        <span class="run-timeline__sequence">{{ item.sequence }}</span>
        <ToolCallCard :item="item" />
      </template>
      <template v-else-if="item.type === 'reasoning'">
        <span class="run-timeline__sequence">{{ item.sequence }}</span>
        <ReasoningDisclosure :text="item.text" />
      </template>
      <template v-else>
        <span class="run-timeline__sequence">{{ item.sequence }}</span>
        <span class="run-timeline__body">{{ itemText(item) }}</span>
      </template>
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

.run-timeline__spacer {
  display: block;
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
