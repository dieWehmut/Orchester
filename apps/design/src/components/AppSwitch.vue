<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    label: string
    id?: string
    describedBy?: string
    invalid?: boolean
    disabled?: boolean
  }>(),
  { invalid: false, disabled: false },
)

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const optionalAttributes = computed<Record<string, string>>(() => {
  const attributes: Record<string, string> = {}
  if (props.id) attributes.id = props.id
  if (props.describedBy) attributes['aria-describedby'] = props.describedBy
  if (props.invalid) attributes['aria-invalid'] = 'true'
  return attributes
})

function toggle(): void {
  if (!props.disabled) emit('update:modelValue', !props.modelValue)
}
</script>

<template>
  <button
    v-bind="optionalAttributes"
    class="app-switch"
    type="button"
    role="switch"
    :aria-checked="modelValue"
    :disabled="disabled"
    @click="toggle"
  >
    <span class="app-switch__track" aria-hidden="true">
      <span class="app-switch__thumb" />
    </span>
    <span class="app-switch__label">{{ label }}</span>
  </button>
</template>

<style scoped>
.app-switch {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-text-primary);
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
}

.app-switch__track {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  width: 36px;
  height: 20px;
  padding: 2px;
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-full);
  background: var(--color-bg-element);
  transition:
    border-color var(--transition-fast) var(--ease-out),
    background var(--transition-fast) var(--ease-out);
}

.app-switch__thumb {
  width: 14px;
  height: 14px;
  border-radius: var(--radius-full);
  background: var(--color-text-tertiary);
  transform: translateX(0);
  transition:
    background var(--transition-fast) var(--ease-out),
    transform var(--transition-fast) var(--ease-out);
}

.app-switch[aria-checked='true'] .app-switch__track {
  border-color: var(--color-accent);
  background: var(--color-accent-muted);
}

.app-switch[aria-checked='true'] .app-switch__thumb {
  background: var(--color-accent);
  transform: translateX(16px);
}

.app-switch[aria-invalid='true'] .app-switch__track {
  border-color: var(--color-status-error);
}

.app-switch:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
</style>
