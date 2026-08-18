<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    lines?: number
    height?: string
    animated?: boolean
  }>(),
  {
    lines: 1,
    height: '1rem',
    animated: true,
  },
)

const reducedMotion = ref(false)
let motionQuery: MediaQueryList | undefined

const lineCount = computed(() => Math.max(1, Math.floor(props.lines)))
const lineStyle = computed<Record<string, string>>(() => ({ height: props.height }))
const isStatic = computed(() => !props.animated || reducedMotion.value)

function onMotionPreferenceChange(event: MediaQueryListEvent) {
  reducedMotion.value = event.matches
}

onMounted(() => {
  if (typeof window.matchMedia !== 'function') {
    return
  }

  motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  reducedMotion.value = motionQuery.matches
  motionQuery.addEventListener('change', onMotionPreferenceChange)
})

onBeforeUnmount(() => {
  motionQuery?.removeEventListener('change', onMotionPreferenceChange)
  motionQuery = undefined
})
</script>

<template>
  <div
    class="skeleton-block"
    :class="{ 'skeleton-block--static': isStatic }"
    aria-hidden="true"
  >
    <span
      v-for="line in lineCount"
      :key="line"
      data-skeleton-line
      class="skeleton-block__line"
      :style="lineStyle"
    />
  </div>
</template>

<style scoped>
.skeleton-block {
  display: grid;
  gap: var(--space-2, 0.5rem);
  inline-size: 100%;
}

.skeleton-block__line {
  display: block;
  inline-size: 100%;
  min-block-size: 4px;
  overflow: hidden;
  border-radius: var(--radius-xs, 4px);
  background: linear-gradient(
    90deg,
    var(--color-bg-element, #1d2129) 25%,
    var(--color-bg-elevated, #242932) 50%,
    var(--color-bg-element, #1d2129) 75%
  );
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.4s linear infinite;
}

.skeleton-block--static .skeleton-block__line {
  animation: none;
}

@keyframes skeleton-shimmer {
  from {
    background-position: 200% 0;
  }

  to {
    background-position: -200% 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .skeleton-block__line {
    animation-duration: 1ms;
    animation-iteration-count: 1;
  }
}
</style>
