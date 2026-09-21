<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    value: number
    max?: number
    label: string
    showValue?: boolean
  }>(),
  {
    max: 100,
    showValue: false,
  },
)

const safeMax = computed(() => (Number.isFinite(props.max) && props.max > 0 ? props.max : 100))
const normalizedValue = computed(() => {
  if (!Number.isFinite(props.value)) {
    return 0
  }
  return Math.min(safeMax.value, Math.max(0, props.value))
})
const percentage = computed(() =>
  Math.round((normalizedValue.value / safeMax.value) * 1000) / 10,
)
const fillStyle = computed<Record<string, string>>(() => ({
  width: percentage.value + '%',
}))
</script>

<template>
  <div
    class="progress-bar"
    role="progressbar"
    aria-valuemin="0"
    :aria-valuemax="safeMax"
    :aria-valuenow="normalizedValue"
    :aria-label="label"
  >
    <div
      class="progress-bar__track"
      data-progress-track
    >
      <span data-progress-fill class="progress-bar__fill" :style="fillStyle" />
    </div>
    <span v-if="showValue" class="progress-bar__value">{{ percentage }}%</span>
  </div>
</template>

<style scoped>
.progress-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2, 0.5rem);
  inline-size: 100%;
}

.progress-bar__track {
  flex: 1;
  min-block-size: 6px;
  overflow: hidden;
  border-radius: var(--radius-full, 9999px);
  background: var(--color-bg-element, #1d2129);
}

.progress-bar__fill {
  display: block;
  min-block-size: inherit;
  border-radius: inherit;
  background: var(--color-accent, #d8a24a);
  transition: width var(--transition-normal, 240ms) var(--ease-out, ease-out);
}

.progress-bar__value {
  min-inline-size: 3.5ch;
  color: var(--color-text-secondary, #a4abb6);
  font-family: var(--font-mono, monospace);
  font-size: var(--text-xs, 0.6875rem);
  text-align: end;
}

@media (prefers-reduced-motion: reduce) {
  .progress-bar__fill {
    transition-duration: 1ms;
  }
}
</style>
