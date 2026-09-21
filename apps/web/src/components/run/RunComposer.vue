<script setup lang="ts">
import { AppButton, AppTextarea, Spinner } from '@orchester/design'
import type { RunLifecycle } from '../../stores/run'
import type { ModelCatalogDto } from '@orchester/protokoll'
import { computed, ref, watch } from 'vue'

import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'
import ApprovalPresetControl, { type ApprovalPreset } from './ApprovalPresetControl.vue'
import { readRunSettings, writeRunSettings, type RunSettings } from './run-settings'
import ComposerContextBar from './ComposerContextBar.vue'
import CommandPalette, { type CommandEntry } from './CommandPalette.vue'
import { COMPOSER_COMMANDS, type ComposerCommand } from './composer-commands'
import { useI18n } from '../../i18n'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    busy?: boolean
    disabled?: boolean
    /** Where the run is in its lifecycle. `busy` is derived from it when given. */
    lifecycle?: RunLifecycle | null
    /** Whether a file drag is hovering the composer right now. */
    dragActive?: boolean
    /** The approval scope for the next run. */
    approvalPreset?: ApprovalPreset
    /**
     * The task these settings belong to. When given, the composer remembers
     * the preset under that task rather than only reporting it upward, which is
     * what makes section 4.6's "persisted per task" true.
     */
    settingsKey?: string | null
    /** The `/` vocabulary available in this deployment. */
    commands?: readonly ComposerCommand[]
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
  }>(),
  {
    modelValue: '',
    busy: false,
    disabled: false,
    lifecycle: null,
    dragActive: false,
    approvalPreset: 'ask',
    settingsKey: null,
    commands: () => COMPOSER_COMMANDS,
    maxLength: 8000,
    workspaceName: null,
    modelCatalog: null,
    modelStatus: 'idle',
  },
)

const { t } = useI18n()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  submit: [prompt: string]
  cancel: []
  'drop-files': [files: File[]]
  'update:approvalPreset': [value: ApprovalPreset]
  'run-command': [id: string]
}>()

/**
 * The task's remembered settings. The composer reports every change upward and,
 * when it knows the task, also writes it under that task's key - a resumed run
 * has to keep the settings it was started with.
 */
const remembered = ref<RunSettings | null>(null)

watch(
  () => props.settingsKey,
  (key) => {
    remembered.value = key ? readRunSettings(key) : null
  },
  { immediate: true },
)

/**
 * The run the caller handed the composer outranks what the task remembered: the
 * caller is showing the settings of the run that is actually open, and a
 * remembered value is only a starting point for the next one.
 */
const approvalPreset = computed(() => props.approvalPreset ?? remembered.value?.approvalPreset ?? 'ask')

function updateApprovalPreset(value: ApprovalPreset): void {
  if (remembered.value) {
    remembered.value = { ...remembered.value, approvalPreset: value }
    if (props.settingsKey) writeRunSettings(props.settingsKey, remembered.value)
  }
  emit('update:approvalPreset', value)
}

const draft = ref(props.modelValue)
const dragActive = ref(false)
/**
 * The palette reads descriptions as text, so the keys become words here,
 * where the locale service is in scope.
 */
const paletteCommands = computed<readonly CommandEntry[]>(() =>
  props.commands.map((command) => ({
    id: command.id,
    name: command.name,
    description: t(command.descriptionKey),
  })),
)

const commandsClosed = ref(false)

watch(
  () => props.modelValue,
  (value) => {
    if (value !== draft.value) draft.value = value
  },
)

/**
 * The palette is open while the draft is a command being typed. Closing it by
 * hand keeps it closed for that draft, so Escape does not fight the watcher.
 */
const paletteOpen = computed(
  () => !commandsClosed.value && /^\/[^\s]*$/.test(draft.value),
)

/** A drag is only about files; dragging selected text is not a drop. */
function dragCarriesFiles(event: DragEvent): boolean {
  return Array.from(event.dataTransfer?.types ?? []).includes('Files')
}

function handleDragEnter(event: DragEvent): void {
  if (!dragCarriesFiles(event)) return
  dragActive.value = true
}

/**
 * Dragging across a child fires `dragleave` for every ancestor it crosses.
 * The drag has only left the composer once the pointer is outside it, so the
 * state clears on the containment test rather than on the event alone.
 */
function handleDragLeave(event: DragEvent): void {
  const next = event.relatedTarget
  const current = event.currentTarget
  if (next instanceof Node && current instanceof Node && current.contains(next)) return
  dragActive.value = false
}

function handleDrop(event: DragEvent): void {
  dragActive.value = false
  emit('drop-files', Array.from(event.dataTransfer?.files ?? []))
}

/**
 * The composer's own state, named rather than derived from whichever control is
 * disabled. `dragging` outranks the lifecycle because a drop target that does
 * not say it is one is a drop target nobody uses.
 */
const composerState = computed(() => {
  if (props.dragActive || dragActive.value) return 'dragging'
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
  commandsClosed.value = false
  emit('update:modelValue', value)
}

function submit(): void {
  if (!canSubmit.value) return
  emit('submit', draft.value)
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && paletteOpen.value) {
    event.preventDefault()
    commandsClosed.value = true
    return
  }
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
    :data-composer-drag-active="props.dragActive || dragActive"
    :data-composer-danger="approvalPreset === 'full-access'"
    @dragenter.prevent="handleDragEnter"
    @dragover.prevent
    @dragleave="handleDragLeave"
    @drop.prevent="handleDrop"
    @submit.prevent="submit"
  >
    <ComposerContextBar
      class="run-composer__commands"
      :workspace-name="props.workspaceName"
      :model-catalog="props.modelCatalog"
      :model-status="props.modelStatus"
    />
    <CommandPalette
      :open="paletteOpen"
      :commands="paletteCommands"
      :query="draft"
      @select="emit('run-command', $event)"
      @close="commandsClosed = true"
    />
    <label class="run-composer__label" for="run-prompt">{{ props.inputLabel ?? t('run.taskPrompt') }}</label>
    <AppTextarea
      id="run-prompt"
      :model-value="draft"
      :placeholder="props.placeholder ?? t('run.describeTask')"
      :max-length="props.maxLength"
      :disabled="props.disabled || isBusy"
      :rows="rowCount"
      @update:model-value="update"
      @keydown="handleKeydown"
    />
    <div class="run-composer__footer" data-composer-footer>
      <span class="run-composer__count" aria-live="polite">
        {{ draft.length }} / {{ props.maxLength }} {{ props.characterCountLabel ?? t('run.characters') }}
      </span>
      <ApprovalPresetControl
        :model-value="approvalPreset"
        @update:model-value="updateApprovalPreset"
      />
      <div class="run-composer__actions">
        <Spinner
          v-if="isBusy"
          data-run-activity
          class="run-composer__activity"
          :size="14"
          :label="props.activityLabel ?? t('run.runInProgress')"
        />
        <AppButton
          v-if="isBusy"
          type="button"
          variant="danger"
          data-composer-action="cancel"
          :aria-label="props.cancelLabel ?? t('run.stop')"
          @click="emit('cancel')"
        >
          {{ props.cancelLabel ?? t('run.stop') }}
        </AppButton>
        <AppButton
          v-else
          type="submit"
          variant="primary"
          data-composer-action="submit"
          :disabled="!canSubmit"
          :aria-label="props.submitLabel ?? t('run.submit')"
        >
          {{ props.submitLabel ?? t('run.submit') }}
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

/* The drop target has to look like one: a drag that leaves the composer
   unchanged reads as a drag the composer did not notice. */
.run-composer[data-composer-drag-active='true'] {
  border-color: var(--color-accent);
  border-style: dashed;
  background: color-mix(in oklab, var(--color-accent) 6%, var(--color-bg-surface));
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
