<script setup lang="ts">
import { AppButton, AppTextarea } from '@orchester/design'
import type { ModelCatalogDto } from '@orchester/protokoll'
import { computed, ref, watch } from 'vue'

import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'
import ComposerContextBar from './ComposerContextBar.vue'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    busy?: boolean
    disabled?: boolean
    maxLength?: number
    placeholder?: string
    submitLabel?: string
    cancelLabel?: string
    inputLabel?: string
    characterCountLabel?: string
    workspaceName?: string | null
    modelCatalog?: ModelCatalogDto | null
    modelStatus?: ModelCatalogStoreStatus
    approvalLabel?: string
  }>(),
  {
    modelValue: '',
    busy: false,
    disabled: false,
    maxLength: 8000,
    placeholder: 'Describe the task',
    submitLabel: 'Run',
    cancelLabel: 'Stop',
    inputLabel: 'Task prompt',
    characterCountLabel: 'characters',
    workspaceName: null,
    modelCatalog: null,
    modelStatus: 'idle',
    approvalLabel: 'Ask for approval',
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  submit: [prompt: string]
  cancel: []
}>()

const draft = ref(props.modelValue)

watch(
  () => props.modelValue,
  (value) => {
    if (value !== draft.value) draft.value = value
  },
)

const canSubmit = computed(
  () =>
    draft.value.trim().length > 0 &&
    draft.value.length <= props.maxLength &&
    !props.busy &&
    !props.disabled,
)

function update(value: string): void {
  draft.value = value
  emit('update:modelValue', value)
}

function submit(): void {
  if (!canSubmit.value) return
  emit('submit', draft.value)
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return
  event.preventDefault()
  submit()
}
</script>

<template>
  <form class="run-composer" data-run-composer @submit.prevent="submit">
    <ComposerContextBar
      :workspace-name="props.workspaceName"
      :model-catalog="props.modelCatalog"
      :model-status="props.modelStatus"
      :approval-label="props.approvalLabel"
    />
    <label class="run-composer__label" for="run-prompt">{{ props.inputLabel }}</label>
    <AppTextarea
      id="run-prompt"
      :model-value="draft"
      :placeholder="props.placeholder"
      :max-length="props.maxLength"
      :disabled="props.disabled || props.busy"
      :rows="4"
      @update:model-value="update"
      @keydown="handleKeydown"
    />
    <div class="run-composer__footer">
      <span class="run-composer__count" aria-live="polite">
        {{ draft.length }} / {{ props.maxLength }} {{ props.characterCountLabel }}
      </span>
      <div class="run-composer__actions">
        <AppButton
          v-if="props.busy"
          type="button"
          variant="danger"
          :aria-label="props.cancelLabel"
          @click="emit('cancel')"
        >
          {{ props.cancelLabel }}
        </AppButton>
        <AppButton
          v-else
          type="submit"
          variant="primary"
          :disabled="!canSubmit"
          :aria-label="props.submitLabel"
        >
          {{ props.submitLabel }}
        </AppButton>
      </div>
    </div>
  </form>
</template>

<style scoped>
.run-composer {
  display: grid;
  gap: var(--space-2);
  padding: var(--space-4);
  border-block-start: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
}

.run-composer__label {
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.run-composer__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.run-composer__count {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.run-composer__actions {
  display: flex;
  gap: var(--space-2);
}

@media (max-width: 640px) {
  .run-composer {
    padding: var(--space-3);
  }

  .run-composer__footer {
    align-items: stretch;
    flex-direction: column;
  }

  .run-composer__actions,
  .run-composer__actions :deep(button) {
    width: 100%;
  }
}
</style>
