<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    label: string
    id?: string
    name?: string
    describedBy?: string
    invalid?: boolean
    required?: boolean
    disabled?: boolean
    indeterminate?: boolean
  }>(),
  {
    invalid: false,
    required: false,
    disabled: false,
    indeterminate: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const inputElement = ref<HTMLInputElement | null>(null)

const optionalAttributes = computed<Record<string, string>>(() => {
  const attributes: Record<string, string> = {}
  if (props.id) attributes.id = props.id
  if (props.name) attributes.name = props.name
  if (props.describedBy) attributes['aria-describedby'] = props.describedBy
  if (props.invalid) attributes['aria-invalid'] = 'true'
  if (props.required) attributes['aria-required'] = 'true'
  return attributes
})

function syncIndeterminate(): void {
  if (inputElement.value) inputElement.value.indeterminate = props.indeterminate
}

onMounted(syncIndeterminate)
watch(() => props.indeterminate, syncIndeterminate)

function onChange(event: Event): void {
  const target = event.target
  if (target instanceof HTMLInputElement) emit('update:modelValue', target.checked)
}
</script>

<template>
  <label class="app-checkbox">
    <input
      ref="inputElement"
      v-bind="optionalAttributes"
      class="app-checkbox__input"
      type="checkbox"
      :checked="modelValue"
      :required="required"
      :disabled="disabled"
      @change="onChange"
    />
    <span class="app-checkbox__label">{{ label }}</span>
  </label>
</template>

<style scoped>
.app-checkbox {
  display: inline-flex;
  align-items: flex-start;
  gap: var(--space-2);
  min-width: 0;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  cursor: pointer;
}

.app-checkbox__input {
  flex: 0 0 auto;
  width: 16px;
  height: 16px;
  margin: 2px 0 0;
  accent-color: var(--color-accent);
}

.app-checkbox__input:focus-visible {
  outline-offset: 3px;
}

.app-checkbox__input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.app-checkbox__input:disabled + .app-checkbox__label {
  opacity: 0.55;
}

.app-checkbox:has(.app-checkbox__input:disabled) {
  cursor: not-allowed;
}
</style>
