<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    id: string
    label: string
    hint?: string
    error?: string
    required?: boolean
  }>(),
  { required: false },
)

defineSlots<{
  default(props: {
    controlId: string
    describedBy: string | undefined
    invalid: boolean
    required: boolean
  }): unknown
}>()

const hintId = computed(() => props.id + '-hint')
const errorId = computed(() => props.id + '-error')
const describedBy = computed<string | undefined>(() => {
  const ids: string[] = []
  if (props.hint) ids.push(hintId.value)
  if (props.error) ids.push(errorId.value)
  return ids.length > 0 ? ids.join(' ') : undefined
})
const invalid = computed(() => Boolean(props.error))
</script>

<template>
  <div class="app-field" :class="{ 'app-field--invalid': invalid }">
    <label class="app-field__label" :for="id">
      <span>{{ label }}</span>
      <span v-if="required" class="app-field__required" aria-hidden="true">*</span>
    </label>

    <div class="app-field__control">
      <slot
        :control-id="id"
        :described-by="describedBy"
        :invalid="invalid"
        :required="required"
      />
    </div>

    <p v-if="hint" :id="hintId" class="app-field__hint">{{ hint }}</p>
    <p v-if="error" :id="errorId" class="app-field__error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.app-field {
  display: grid;
  gap: var(--space-2);
  min-width: 0;
}

.app-field__label {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
  width: fit-content;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.app-field__required {
  color: var(--color-status-error);
}

.app-field__control {
  min-width: 0;
}

.app-field__hint,
.app-field__error {
  margin: 0;
  font-size: var(--text-xs);
  line-height: var(--leading-normal);
}

.app-field__hint {
  color: var(--color-text-tertiary);
}

.app-field__error {
  color: var(--color-status-error);
}
</style>
