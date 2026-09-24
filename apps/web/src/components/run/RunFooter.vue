<script setup lang="ts">
import type { RunView } from '@orchester/ereignis'

/**
 * The run's ledger: which sequence it reached and what it spent.
 *
 * The clock belongs to the answer it measured and to the strip while a run is in
 * flight, so it is not repeated here.
 */
defineProps<{
  view: RunView
  sequenceLabel?: string
  usageLabel?: string
}>()
</script>

<template>
  <footer class="run-footer" data-run-footer :aria-label="usageLabel ?? 'Run usage'">
    <span>{{ sequenceLabel ?? 'Sequence' }} {{ view.latestSequence }}</span>
    <span>{{ usageLabel ?? 'Usage' }}</span>
    <span>in {{ view.usage.input_tokens }}</span>
    <span>out {{ view.usage.output_tokens }}</span>
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
