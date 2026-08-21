<script setup lang="ts">
import { computed } from 'vue'

type WrapMode = 'hard' | 'off' | 'soft'

const props = withDefaults(
  defineProps<{
    modelValue: string
    id?: string
    name?: string
    placeholder?: string
    rows?: number
    wrap?: WrapMode
    describedBy?: string
    invalid?: boolean
    required?: boolean
    maxLength?: number
    disabled?: boolean
    readonly?: boolean
  }>(),
  {
    rows: 4,
    wrap: 'soft',
    invalid: false,
    required: false,
    disabled: false,
    readonly: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const optionalAttributes = computed<Record<string, string>>(() => {
  const attributes: Record<string, string> = {}
  if (props.id) attributes.id = props.id
  if (props.name) attributes.name = props.name
  if (props.placeholder) attributes.placeholder = props.placeholder
  if (props.describedBy) attributes['aria-describedby'] = props.describedBy
  if (props.invalid) attributes['aria-invalid'] = 'true'
  if (props.required) attributes['aria-required'] = 'true'
  if (props.maxLength !== undefined) attributes.maxlength = String(props.maxLength)
  return attributes
})

function onInput(event: Event): void {
  const target = event.target
  if (target instanceof HTMLTextAreaElement) emit('update:modelValue', target.value)
}
</script>

<template>
  <textarea
    v-bind="optionalAttributes"
    class="app-textarea"
    :rows="rows"
    :wrap="wrap"
    :value="modelValue"
    :required="required"
    :disabled="disabled"
    :readonly="readonly"
    @input="onInput"
  />
</template>

<style scoped>
.app-textarea {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: var(--control-height-lg);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-input);
  color: var(--color-text-primary);
  font: inherit;
  line-height: var(--leading-normal);
  resize: vertical;
  transition:
    border-color var(--transition-fast) var(--ease-out),
    background var(--transition-fast) var(--ease-out);
}

.app-textarea::placeholder {
  color: var(--color-text-tertiary);
}

.app-textarea:hover:not(:disabled) {
  border-color: var(--color-border-strong);
}

.app-textarea:focus-visible {
  border-color: var(--color-accent);
}

.app-textarea[aria-invalid='true'] {
  border-color: var(--color-status-error);
}

.app-textarea:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.app-textarea:read-only {
  background: var(--color-bg-element);
}
</style>
