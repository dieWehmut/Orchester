<script setup lang="ts">
import { computed } from 'vue'

import type { AppSelectOption } from './form-types'

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: readonly AppSelectOption[]
    id?: string
    name?: string
    placeholder?: string
    describedBy?: string
    invalid?: boolean
    required?: boolean
    disabled?: boolean
  }>(),
  { invalid: false, required: false, disabled: false },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const optionalAttributes = computed<Record<string, string>>(() => {
  const attributes: Record<string, string> = {}
  if (props.id) attributes.id = props.id
  if (props.name) attributes.name = props.name
  if (props.describedBy) attributes['aria-describedby'] = props.describedBy
  if (props.invalid) attributes['aria-invalid'] = 'true'
  if (props.required) attributes['aria-required'] = 'true'
  return attributes
})

function onChange(event: Event): void {
  const target = event.target
  if (target instanceof HTMLSelectElement) emit('update:modelValue', target.value)
}
</script>

<template>
  <select
    v-bind="optionalAttributes"
    class="app-select"
    :value="modelValue"
    :required="required"
    :disabled="disabled"
    @change="onChange"
  >
    <option v-if="placeholder" value="" disabled>{{ placeholder }}</option>
    <option
      v-for="option in options"
      :key="option.value"
      :value="option.value"
      :disabled="option.disabled === true"
    >
      {{ option.label }}
    </option>
  </select>
</template>

<style scoped>
.app-select {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: var(--control-height-md);
  padding: 0 var(--space-8) 0 var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background:
    var(--color-bg-input)
    linear-gradient(45deg, transparent 50%, var(--color-text-tertiary) 50%) calc(100% - 16px)
      15px / 6px 6px no-repeat,
    linear-gradient(135deg, var(--color-text-tertiary) 50%, transparent 50%) calc(100% - 12px)
      15px / 6px 6px no-repeat;
  color: var(--color-text-primary);
  font: inherit;
  cursor: pointer;
  appearance: none;
  transition:
    border-color var(--transition-fast) var(--ease-out),
    background-color var(--transition-fast) var(--ease-out);
}

.app-select:hover:not(:disabled) {
  border-color: var(--color-border-strong);
}

.app-select:focus-visible {
  border-color: var(--color-accent);
}

.app-select[aria-invalid='true'] {
  border-color: var(--color-status-error);
}

.app-select:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
</style>
