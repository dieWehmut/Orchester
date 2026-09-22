<script setup lang="ts">
import type { RunView, TimelineItem } from '@orchester/ereignis'
import { MarkdownText } from '@orchester/design'
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import { useI18n } from '../../i18n'
import MessageActions from './MessageActions.vue'
import ReasoningDisclosure from './ReasoningDisclosure.vue'
import ToolCallCard from './ToolCallCard.vue'
import { arrivalState, streamingContainment, type ArrivalState } from './streaming-text'
import { virtualWindow } from './virtual-window'

const { t } = useI18n()

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

/**
 * The arrival mark for a row. Only messages arrive word by word, so only they
 * carry one; a tool card is settled the moment it is written.
 */
function rowArrival(item: TimelineItem): ArrivalState {
  return item.type === 'message' ? arrivalState(item) : 'settled'
}

/**
 * The shape a row is drawn in.
 *
 * Two idioms share this list: the conversation, which is prose and a bubble,
 * and the run's own record, which is cards and ledger numbers. A reader looking
 * for the answer should not have to read past a sequence number to find it, and
 * an auditor looking for the fourth event should still find it on the event.
 */
function rowShape(item: TimelineItem): 'prose' | 'bubble' | 'record' {
  if (item.type === 'message') return item.role === 'user' ? 'bubble' : 'prose'
  if (item.type === 'reasoning') return 'prose'
  return 'record'
}

function rowStyle(item: TimelineItem): Record<string, string> {
  return rowArrival(item) === 'streaming' ? { ...streamingContainment } : {}
}

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
      :data-item-role="item.type === 'message' ? item.role : undefined"
      :data-message-shape="rowShape(item)"
      :data-virtualized-turn="windowRange.start + index"
      :data-arrival-state="rowArrival(item)"
      :style="rowStyle(item)"
    >
      <template v-if="item.type === 'tool'">
        <span class="run-timeline__sequence" data-run-sequence>{{ item.sequence }}</span>
        <ToolCallCard :item="item" />
      </template>
      <template v-else-if="item.type === 'reasoning'">
        <ReasoningDisclosure :text="item.text" />
      </template>
      <template v-else-if="item.type === 'message'">
        <div class="run-timeline__message" data-message-body>
          <!--
            The answer renders as the markdown it was written in, once it has
            settled. While it is still arriving the text stays literal: a fence
            that is half written is not a code block yet, and formatting it
            line by line would flicker the paragraph apart under the reader.
          -->
          <MarkdownText
            v-if="item.role === 'assistant' && rowArrival(item) !== 'streaming'"
            :text="item.text"
            :external-label="t('transcript.externalLink')"
          />
          <p v-else class="run-timeline__text" data-message-plain>{{ item.text }}</p>
          <!--
            Only the answer carries a copy: the reader already has their own
            words, and the reference hangs the actions off the answer too.
          -->
          <MessageActions
            v-if="item.role === 'assistant'"
            class="run-timeline__actions"
            :text="item.text"
            :label="t('transcript.copyMessage')"
            :copied-label="t('transcript.copied')"
          />
        </div>
      </template>
      <template v-else>
        <span class="run-timeline__sequence" data-run-sequence>{{ item.sequence }}</span>
        <span class="run-timeline__body">{{ itemText(item) }}</span>
      </template>
    </li>
  </ol>
</template>

<style scoped>
.run-timeline {
  display: grid;
  gap: var(--space-2);
  /* The conversation keeps a measure, as the reference keeps one: a governed
     run can print a lot of record, and prose that spans the window is prose
     nobody reads. */
  inline-size: min(100%, 52rem);
  margin-inline: auto;
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

/* The conversation is not a card: it is what the reader came to read, and the
   border, the surface and the ledger number are all chrome the answer does not
   need. Everything the run *did* keeps them. */
.run-timeline__item[data-message-shape='prose'] {
  grid-template-columns: minmax(0, 1fr);
  gap: var(--space-1);
  padding-inline: 0;
  border-inline-start: 0;
  background: transparent;
}

.run-timeline__item[data-message-shape='bubble'] {
  grid-template-columns: minmax(0, 1fr);
  justify-items: end;
  padding-inline: 0;
  border-inline-start: 0;
  background: transparent;
}

.run-timeline__message {
  display: grid;
  gap: var(--space-1);
  min-inline-size: 0;
  inline-size: 100%;
}

.run-timeline__text {
  margin: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

/* The reader's own turn, filled: the reference answers a question with a shape
   rather than a label, and the solid pair is the one token set that promises
   its own text stays readable on it. */
.run-timeline__item[data-message-shape='bubble'] .run-timeline__message {
  inline-size: fit-content;
  max-inline-size: min(100%, 40rem);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-lg);
  background: var(--color-action-solid);
  color: var(--color-action-contrast);
}

/* Offered on hover or focus, never removed from the tree: a control that exists
   only under a pointer is a control a keyboard user cannot reach. */
.run-timeline__actions {
  opacity: 0;
  transition: opacity var(--transition-fast) var(--ease-out);
}

.run-timeline__message:hover .run-timeline__actions,
.run-timeline__message:focus-within .run-timeline__actions {
  opacity: 1;
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
