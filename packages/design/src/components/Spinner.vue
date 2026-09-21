<script setup lang="ts">
/** An indeterminate progress indicator. */
withDefaults(defineProps<{ label?: string; size?: number }>(), {
  label: 'Loading',
  size: 16,
})
</script>

<template>
  <span
    class="spinner"
    role="status"
    :aria-label="label"
    :style="{ width: `${size}px`, height: `${size}px` }"
  />
</template>

<style scoped>
.spinner {
  display: inline-block;
  flex: none;
  border: 2px solid var(--color-border-strong);
  border-top-color: var(--color-accent);
  border-radius: var(--radius-full);
  animation: spinner-turn 720ms linear infinite;
}

@keyframes spinner-turn {
  to {
    transform: rotate(360deg);
  }
}

/* Under reduced motion the global rule pins the animation to a single 1ms
   iteration, which would freeze the ring at its start angle and read as a broken
   image. A pulse carries "busy" without rotation. */
@media (prefers-reduced-motion: reduce) {
  .spinner {
    animation: spinner-fade 1.2s ease-in-out infinite !important;
    animation-iteration-count: infinite !important;
    border-top-color: var(--color-accent);
  }

  @keyframes spinner-fade {
    0%,
    100% {
      opacity: 1;
    }

    50% {
      opacity: 0.4;
    }
  }
}
</style>
