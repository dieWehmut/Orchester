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

function choose(index: number): void {
  emit('select', index)
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
    @pointerdown="scrubbing = true"
    @pointerup="scrubbing = false"
    @pointerleave="scrubbing = false"
  >
    <button
      v-for="mark in marks"
      :key="mark.index"
      class="message-rail__mark"
      type="button"
      data-rail-mark
      :data-rail-index="mark.index"
      :aria-label="mark.label"
      @click="choose(mark.index)"
      @pointerenter="hovered = mark.index"
      @pointerleave="hovered = null"
      @focus="hovered = mark.index"
      @blur="hovered = null"
    >
      <span
        v-if="hovered === mark.index"
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
