<script setup lang="ts">
import mark from '../../assets/orchester-mark.png'

withDefaults(
  defineProps<{
    size?: number
  }>(),
  { size: 104 },
)
</script>

<template>
  <span
    class="orchester-mark"
    data-orchester-mark
    aria-hidden="true"
    :style="{ inlineSize: `${size}px`, blockSize: `${size}px` }"
  >
    <img
      class="orchester-mark__art"
      :src="mark"
      :width="size"
      :height="size"
      alt=""
      draggable="false"
    />
  </span>
</template>

<style scoped>
.orchester-mark {
  display: grid;
  place-items: center;
  overflow: hidden;
  border: 1px solid var(--color-border-base);
  border-radius: 28px;
  background: var(--color-bg-surface);
  box-shadow: 0 18px 48px rgb(0 0 0 / 22%);
}

.orchester-mark__art {
  inline-size: 100%;
  block-size: 100%;
  object-fit: cover;
  user-select: none;
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