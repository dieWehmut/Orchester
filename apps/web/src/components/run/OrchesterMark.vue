<script setup lang="ts">
import { Orbit } from '@lucide/vue'

withDefaults(
  defineProps<{
    size?: number
  }>(),
  { size: 72 },
)
</script>

<template>
  <span class="orchester-mark" data-orchester-mark aria-hidden="true">
    <Orbit :size="size" :stroke-width="1.35" />
  </span>
</template>

<style scoped>
.orchester-mark {
  display: grid;
  inline-size: 104px;
  block-size: 104px;
  place-items: center;
  border: 1px solid var(--color-accent-border);
  border-radius: 28px;
  background: var(--color-bg-surface);
  color: var(--color-accent);
  box-shadow: 0 18px 48px rgb(0 0 0 / 22%);
}

@media (prefers-reduced-motion: no-preference) {
  .orchester-mark {
    animation: mark-in 420ms var(--ease-out) both;
  }
}

@keyframes mark-in {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.94);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
</style>
