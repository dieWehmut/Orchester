<script setup lang="ts">
/**
 * A square button whose whole content is an icon.
 *
 * `label` is required and not optional. An icon-only control with no accessible
 * name is invisible to a screen reader, and making the prop optional means the
 * accessible name is the first thing dropped under deadline.
 */
withDefaults(
  defineProps<{
    label: string
    active?: boolean
    disabled?: boolean
  }>(),
  { active: false, disabled: false },
)

defineEmits<{ click: [event: MouseEvent] }>()
</script>

<template>
  <button
    class="icon-button"
    :class="{ 'icon-button--active': active }"
    type="button"
    :disabled="disabled"
    :title="label"
    :aria-label="label"
    :aria-pressed="active"
    @click="$emit('click', $event)"
  >
    <slot />
  </button>
</template>

<style scoped>
.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition:
    background var(--transition-fast) var(--ease-out),
    color var(--transition-fast) var(--ease-out);
}

.icon-button:hover:not(:disabled) {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.icon-button--active {
  background: var(--color-accent-muted);
  border-color: var(--color-accent-border);
  color: var(--color-accent);
}

.icon-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
