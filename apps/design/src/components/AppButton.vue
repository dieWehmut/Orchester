<script setup lang="ts">
/**
 * The one button.
 *
 * `variant` covers the three jobs a button has in this app — commit, choose,
 * abandon — rather than describing a colour, so a redesign is a change here and
 * nowhere else. `busy` is separate from `disabled` because a busy button must
 * stay focused: moving focus to the body mid-action loses a keyboard user's place.
 * It reports as `aria-busy="false"` when idle, which is what an absent
 * `aria-busy` means anyway.
 */
import { computed } from 'vue'

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger'
type Size = 'sm' | 'md'

const props = withDefaults(
  defineProps<{
    variant?: Variant
    size?: Size
    disabled?: boolean
    busy?: boolean
    type?: 'button' | 'submit'
    block?: boolean
  }>(),
  { variant: 'primary', size: 'md', disabled: false, busy: false, type: 'button', block: false },
)

defineEmits<{ click: [event: MouseEvent] }>()

const classes = computed(() => [
  'app-button',
  `app-button--${props.variant}`,
  `app-button--${props.size}`,
  { 'app-button--block': props.block, 'app-button--busy': props.busy },
])
</script>

<template>
  <button
    :class="classes"
    :type="type"
    :disabled="disabled"
    :aria-busy="busy"
    @click="$emit('click', $event)"
  >
    <span class="app-button__label"><slot /></span>
  </button>
</template>

<style scoped>
.app-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition:
    background var(--transition-fast) var(--ease-out),
    border-color var(--transition-fast) var(--ease-out),
    color var(--transition-fast) var(--ease-out);
}

.app-button--sm {
  padding: var(--space-1) var(--space-3);
  font-size: var(--text-sm);
}

.app-button--md {
  padding: var(--space-2) var(--space-4);
  font-size: var(--text-base);
}

.app-button--block {
  width: 100%;
}

.app-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.app-button--busy {
  cursor: progress;
}

.app-button--primary {
  background: var(--color-accent);
  color: var(--color-accent-contrast);
}

.app-button--primary:hover:not(:disabled) {
  background: var(--color-accent-hover);
}

.app-button--secondary {
  background: var(--color-bg-element);
  border-color: var(--color-border-base);
  color: var(--color-text-primary);
}

.app-button--secondary:hover:not(:disabled) {
  border-color: var(--color-border-strong);
  background: var(--color-bg-elevated);
}

.app-button--ghost {
  background: transparent;
  color: var(--color-text-secondary);
}

.app-button--ghost:hover:not(:disabled) {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.app-button--danger {
  background: transparent;
  border-color: var(--color-status-error);
  color: var(--color-status-error);
}

.app-button--danger:hover:not(:disabled) {
  background: var(--color-status-error);
  color: var(--color-text-inverse);
}
</style>
