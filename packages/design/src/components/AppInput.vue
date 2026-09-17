<script setup lang="ts">
import { computed } from 'vue'

type InputType = 'email' | 'number' | 'password' | 'search' | 'text' | 'url'

const props = withDefaults(
  defineProps<{
    modelValue: string
    id?: string
    name?: string
    type?: InputType
    placeholder?: string
    autocomplete?: string
    describedBy?: string
    invalid?: boolean
    required?: boolean
    disabled?: boolean
    readonly?: boolean
  }>(),
  { type: 'text', invalid: false, required: false, disabled: false, readonly: false },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const optionalAttributes = computed<Record<string, string>>(() => {
  const attributes: Record<string, string> = {}
  if (props.id) attributes.id = props.id
  if (props.name) attributes.name = props.name
  if (props.placeholder) attributes.placeholder = props.placeholder
  if (props.autocomplete) attributes.autocomplete = props.autocomplete
  if (props.describedBy) attributes['aria-describedby'] = props.describedBy
  if (props.invalid) attributes['aria-invalid'] = 'true'
  if (props.required) attributes['aria-required'] = 'true'
  return attributes
})

function onInput(event: Event): void {
  const target = event.target
  if (target instanceof HTMLInputElement) emit('update:modelValue', target.value)
}
</script>

<template>
  <input
    v-bind="optionalAttributes"
    class="app-input"
    :type="type"
    :value="modelValue"
    :required="required"
    :disabled="disabled"
    :readonly="readonly"
    @input="onInput"
  />
</template>

<style scoped>
.app-input {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: var(--control-height-md);
  padding: 0 var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-input);
  color: var(--color-text-primary);
  font: inherit;
  line-height: var(--leading-normal);
  transition:
    border-color var(--transition-fast) var(--ease-out),
    background var(--transition-fast) var(--ease-out);
}

.app-input::placeholder {
  color: var(--color-text-tertiary);
}

.app-input:hover:not(:disabled) {
  border-color: var(--color-border-strong);
}

.app-input:focus-visible {
  border-color: var(--color-accent);
}

.app-input[aria-invalid='true'] {
  border-color: var(--color-status-error);
}

.app-input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.app-input:read-only {
  background: var(--color-bg-element);
}
</style>
