<script setup lang="ts">
withDefaults(
  defineProps<{
    focusable?: boolean
  }>(),
  {
    focusable: false,
  },
)
</script>

<template>
  <span
    class="visually-hidden"
    :class="{ 'visually-hidden--focusable': focusable }"
  >
    <slot />
  </span>
</template>

<style scoped>
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  border: 0;
  clip: rect(0 0 0 0);
  white-space: nowrap;
}

.visually-hidden--focusable:focus,
.visually-hidden--focusable:focus-visible {
  position: static;
  width: auto;
  height: auto;
  margin: 0;
  overflow: visible;
  clip: auto;
  white-space: normal;
}
</style>
