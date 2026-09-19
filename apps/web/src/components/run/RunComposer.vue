<script setup lang="ts">
import { AppButton, AppTextarea, Spinner } from '@orchester/design'
import type { RunLifecycle } from '../../stores/run'
import type { ModelCatalogDto } from '@orchester/protokoll'
import { computed, ref, watch } from 'vue'

import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'
import ComposerContextBar from './ComposerContextBar.vue'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    busy?: boolean
    disabled?: boolean
    /** Where the run is in its lifecycle. `busy` is derived from it when given. */
    lifecycle?: RunLifecycle | null
    /** Whether a file drag is hovering the composer right now. */
    dragActive?: boolean
    maxLength?: number
    placeholder?: string
    submitLabel?: string
    cancelLabel?: string
    activityLabel?: string
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
    lifecycle: null,
    dragActive: false,
    maxLength: 8000,
    placeholder: 'Describe the task',
    submitLabel: 'Run',
    cancelLabel: 'Stop',
    activityLabel: 'Run in progress',
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

/**
 * The composer's own state, named rather than derived from whichever control is
 * disabled. `dragging` outranks the lifecycle because a drop target that does
 * not say it is one is a drop target nobody uses.
 */
const composerState = computed(() => {
  if (props.dragActive) return 'dragging'
  if (props.lifecycle === 'submitting') return 'submitting'
  if (props.lifecycle === 'running') return 'running'
  if (props.lifecycle === 'cancelling') return 'cancelling'
  return 'idle'
})

/**
 * `busy` is the control-level truth (an input that must not accept a second
 * prompt); the lifecycle names why. Deriving it here keeps the two from
 * drifting: a caller that knows the lifecycle does not also have to remember
 * to set `busy`.
 */
const isBusy = computed(
  () =>
    props.busy ||
    props.lifecycle === 'submitting' ||
    props.lifecycle === 'running' ||
    props.lifecycle === 'cancelling',
)

/**
 * Auto-grow between one and twelve rows, counting wrapped lines as well as
 * explicit newlines. A fixed box either wastes space on a one-line prompt or
 * scrolls a long one out of sight while the user is still writing it.
 */
const MIN_ROWS = 1
const MAX_ROWS = 12

const rowCount = computed(() => {
  const explicit = draft.value.split('\n').length
  // Long single lines wrap; 72 characters is the estimate the measure implies.
  const wrapped = draft.value
    .split('\n')
    .reduce((total, line) => total + Math.max(1, Math.ceil(line.length / 72)), 0)
  return Math.min(MAX_ROWS, Math.max(MIN_ROWS, Math.max(explicit, wrapped)))
})

const canSubmit = computed(
  () =>
    draft.value.trim().length > 0 &&
    draft.value.length <= props.maxLength &&
    !isBusy.value &&
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
  <form
    class="run-composer"
    data-run-composer
    :data-composer-state="composerState"
    @submit.prevent="submit"
  >
    <ComposerContextBar
      class="run-composer__commands"
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
      :disabled="props.disabled || isBusy"
      :rows="rowCount"
      @update:model-value="update"
      @keydown="handleKeydown"
    />
    <div class="run-composer__footer" data-composer-footer>
      <span class="run-composer__count" aria-live="polite">
        {{ draft.length }} / {{ props.maxLength }} {{ props.characterCountLabel }}
      </span>
      <div class="run-composer__actions">
        <Spinner
          v-if="isBusy"
          data-run-activity
          class="run-composer__activity"
          :size="14"
          :label="props.activityLabel"
        />
        <AppButton
          v-if="isBusy"
          type="button"
          variant="danger"
          data-composer-action="cancel"
          :aria-label="props.cancelLabel"
          @click="emit('cancel')"
        >
          {{ props.cancelLabel }}
        </AppButton>
        <AppButton
          v-else
          type="submit"
          variant="primary"
          data-composer-action="submit"
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
  max-inline-size: 54rem;
  margin-inline: auto;
  padding: var(--space-4);
  border: 1px solid var(--color-border-base);
  border-radius: 1.25rem;
  background: var(--color-bg-surface);
  box-shadow: 0 12px 34px rgb(0 0 0 / 14%);
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

.run-composer__commands {
  min-inline-size: 0;
  padding: 0 0 var(--space-2);
  border-block-end: 1px solid var(--color-border-base);
}

.run-composer :deep(.app-textarea) {
  min-block-size: 5rem;
  border-color: transparent;
  background: var(--color-bg-element);
  border-radius: 0.875rem;
}

.run-composer :deep(.app-textarea:focus-visible) {
  border-color: var(--color-accent);
}

.run-composer__count {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.run-composer__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.run-composer__activity {
  margin-inline-end: var(--space-1);
}

@media (max-width: 640px) {
  .run-composer {
    padding: var(--space-3);
    border-radius: 1rem;
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
