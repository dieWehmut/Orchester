<script setup lang="ts">
import type { RunView, TimelineItem } from '@orchester/ereignis'
import { MarkdownText } from '@orchester/design'
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import { useI18n } from '../../i18n'
import MessageActions from './MessageActions.vue'
import ReasoningDisclosure from './ReasoningDisclosure.vue'
import ToolCallCard from './ToolCallCard.vue'
import { arrivalState, streamingContainment, type ArrivalState } from './streaming-text'
import { dayLabel, startsDay } from './transcript-days'
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

/**
 * The reader's next move, reported upward rather than performed here: this list
 * draws a timeline, and the composer is the surface that owns a draft.
 */
const emit = defineEmits<{
  quote: [text: string]
  reuse: [text: string]
}>()

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
 * The rows on screen, each knowing whether it opens a day.
 *
 * The mark is drawn inside the row rather than between rows: the list measures
 * its rows by their position, and a mark that were its own child would move
 * every measurement below it. Asked of the whole timeline, so scrolling the
 * window does not invent a mark in the middle of a day.
 */
const mountedRows = computed(() =>
  mounted.value.map((item, index) => ({
    item,
    // The label and the instant are worked out here rather than in the template
    // because a row the journal left without a time has neither.
    day: dayLabel(item.occurredAt),
    datetime: item.occurredAt ?? '',
    startsDay: startsDay(props.view.timeline, windowRange.value.start + index),
  })),
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
      v-for="(row, index) in mountedRows"
      :key="row.item.key"
      class="run-timeline__item"
      :class="`run-timeline__item--${row.item.type}`"
      :data-item-type="row.item.type"
      :data-item-role="row.item.type === 'message' ? row.item.role : undefined"
      :data-message-shape="rowShape(row.item)"
      :data-virtualized-turn="windowRange.start + index"
      :data-arrival-state="rowArrival(row.item)"
      :style="rowStyle(row.item)"
    >
      <!--
        Where the stream crossed midnight, in the reference's own mark: the day
        and the time of the first row that belongs to it.
      -->
      <div v-if="row.startsDay" class="run-timeline__day" data-day-separator>
        <time :datetime="row.datetime">{{ row.day }}</time>
      </div>
      <template v-if="row.item.type === 'tool'">
        <span class="run-timeline__sequence" data-run-sequence>{{ row.item.sequence }}</span>
        <ToolCallCard :item="row.item" />
      </template>
      <template v-else-if="row.item.type === 'reasoning'">
        <ReasoningDisclosure :text="row.item.text" />
      </template>
      <template v-else-if="row.item.type === 'message'">
        <div class="run-timeline__message" data-message-body>
          <!--
            The answer renders as the markdown it was written in, from the first
            token: the reference formats while it is still being written, and an
            unclosed fence reads as the code it already is. The reader's own turn
            is not parsed - their words are not the agent's markup - and the row
            carries `contain: layout paint` while it arrives, which is what keeps
            a growing row from being measured by the transcript's own layout.
          -->
          <MarkdownText
            v-if="row.item.role === 'assistant'"
            :text="row.item.text"
            :external-label="t('transcript.externalLink')"
          />
          <p v-else class="run-timeline__text" data-message-plain>{{ row.item.text }}</p>
          <!--
            Each side of the conversation offers the move that belongs to it:
            the answer can be quoted into the composer as the start of the next
            prompt, and the reader's own turn can be put back to run again.
            Neither starts work on its own - both end with the caret in the
            field - which is what the reference's own row does not promise.
          -->
          <MessageActions
            v-if="row.item.role === 'assistant'"
            class="run-timeline__actions"
            :text="row.item.text"
            :label="t('transcript.copyMessage')"
            :copied-label="t('transcript.copied')"
            :actions="[
              { id: 'quote', label: t('transcript.quote'), icon: 'quote' as const },
            ]"
            @action="emit('quote', row.item.text)"
          />
          <MessageActions
            v-else
            class="run-timeline__actions"
            :text="row.item.text"
            :label="t('transcript.copyMessage')"
            :copied-label="t('transcript.copied')"
            :actions="[
              { id: 'reuse', label: t('transcript.reuse'), icon: 'reuse' as const },
            ]"
            @action="emit('reuse', row.item.text)"
          />
        </div>
      </template>
      <template v-else>
        <span class="run-timeline__sequence" data-run-sequence>{{ row.item.sequence }}</span>
        <span class="run-timeline__body">{{ itemText(row.item) }}</span>
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

/* Where the stream crossed midnight, in the reference's own mark: centred over
   both columns, because it belongs to the column rather than to the row's
   sequence number. */
.run-timeline__day {
  display: flex;
  grid-column: 1 / -1;
  /* The bubble row aligns its items to the end; the mark belongs to the column
     rather than to either side of the conversation, so it stretches across the
     row and centres itself in it. */
  justify-self: stretch;
  align-items: center;
  justify-content: center;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
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
