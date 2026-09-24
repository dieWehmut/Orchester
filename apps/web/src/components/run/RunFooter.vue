<script setup lang="ts">
import type { RunView } from '@orchester/ereignis'
import { computed } from 'vue'

import { runElapsed } from './run-duration'

const props = defineProps<{
  view: RunView
  sequenceLabel?: string
  usageLabel?: string
  /**
   * What to call the run's elapsed time, with the time already in it.
   *
   * The label is passed in rather than composed here: it is a sentence, and
   * every sentence in this product comes from the locale catalogues. It is
   * optional *and* nullable, because a run that has not taken a measurable
   * moment has no sentence to pass.
   */
  durationLabel?: string | undefined
}>()

/** Only worth stating once the run has taken a measurable moment. */
const elapsed = computed(() => runElapsed(props.view))
</script>

<template>
  <footer class="run-footer" data-run-footer :aria-label="usageLabel ?? 'Run usage'">
    <span>{{ sequenceLabel ?? 'Sequence' }} {{ view.latestSequence }}</span>
    <span>{{ usageLabel ?? 'Usage' }}</span>
    <span>in {{ view.usage.input_tokens }}</span>
    <span>out {{ view.usage.output_tokens }}</span>
    <span v-if="elapsed !== null && durationLabel" data-run-duration>{{ durationLabel }}</span>
  </footer>
</template>

<style scoped>
.run-footer {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  border-block-start: 1px solid var(--color-border-base);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}
</style>
