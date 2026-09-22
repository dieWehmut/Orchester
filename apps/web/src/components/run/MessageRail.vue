<script setup lang="ts">
import type { RunView } from '@orchester/ereignis'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'
import { railMarks } from './message-rail'

const { t } = useI18n()

const props = defineProps<{
  view: RunView
}>()

const emit = defineEmits<{
  /** The transcript index the reader chose. */
  select: [index: number]
}>()

const marks = computed(() => railMarks(props.view.timeline))

/** Which mark is showing its label, if any. */
const hovered = ref<number | null>(null)

/** True while the reader is dragging the rail to scrub the transcript. */
const scrubbing = ref(false)

/**
 * Where the transcript was sent, and where a drag currently is.
 *
 * They differ for as long as a drag takes, and that is exactly when the reader
 * needs the words: aiming at a mark means choosing a turn by its question.
 */
const active = ref<number | null>(null)
const aiming = ref<number | null>(null)

/** The mark showing its label: the one being aimed at, or the one hovered. */
const labelled = computed(() => aiming.value ?? hovered.value)

function choose(index: number): void {
  active.value = index
  emit('select', index)
}

function startScrub(): void {
  scrubbing.value = true
}

/**
 * The pointer says which mark it is over; the rail turns that into the jump.
 *
 * A drag that only raised a flag would move nothing, so each mark the finger
 * reaches is a destination. Re-reporting the mark already reached is ignored,
 * because otherwise the transcript would re-scroll on every pixel of the drag.
 */
function scrubTo(index: number): void {
  if (!scrubbing.value) return
  aiming.value = index
  if (active.value === index) return
  active.value = index
  emit('select', index)
}

function endScrub(): void {
  scrubbing.value = false
  aiming.value = null
}
</script>

<template>
  <div
    v-if="marks.length > 0"
    class="message-rail"
    data-message-rail
    :data-scrubbing="String(scrubbing)"
    role="navigation"
    :aria-label="t('transcript.messageNavigation')"
    @pointerdown="startScrub"
    @pointerup="endScrub"
    @pointerleave="endScrub"
    @pointercancel="endScrub"
  >
    <button
      v-for="mark in marks"
      :key="mark.index"
      class="message-rail__mark"
      type="button"
      data-rail-mark
      :data-rail-index="mark.index"
      :data-rail-active="active === mark.index ? '' : undefined"
      :aria-label="mark.label"
      @click="choose(mark.index)"
      @pointermove="scrubTo(mark.index)"
      @pointerenter="hovered = mark.index"
      @pointerleave="hovered = null"
      @focus="hovered = mark.index"
      @blur="hovered = null"
    >
      <span
        v-if="labelled === mark.index"
        class="message-rail__label"
        data-rail-label
      >{{ mark.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.message-rail {
  position: absolute;
  inset-block: var(--space-6);
  inset-inline-end: var(--space-2);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  justify-content: center;
  pointer-events: auto;
}

.message-rail__mark {
  position: relative;
  min-block-size: var(--hit-target-min, 32px);
  min-inline-size: var(--hit-target-min, 32px);
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.message-rail__mark::before {
  position: absolute;
  inset-block-start: 50%;
  inset-inline-end: var(--space-3);
  inline-size: var(--space-4);
  block-size: 2px;
  border-radius: var(--radius-full);
  background: var(--color-border-strong);
  content: '';
  translate: 0 -50%;
}

.message-rail__label {
  position: absolute;
  inset-block-start: 50%;
  inset-inline-end: calc(100% - var(--space-1));
  max-inline-size: 240px;
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  text-align: end;
  translate: 0 -50%;
  white-space: nowrap;
}
</style>
