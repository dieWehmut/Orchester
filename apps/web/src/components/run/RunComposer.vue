<script setup lang="ts">
import { AppButton, AppTextarea, Spinner } from '@orchester/design'
import type { RunLifecycle } from '../../stores/run'
import type { ModelCatalogDto, ModelSelectionRequestDto } from '@orchester/protokoll'
import { ArrowUp, Square, X, FileText, Mic, MicOff } from '@lucide/vue'
import { computed, ref, watch } from 'vue'

import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'
import ApprovalPresetControl, { type ApprovalPreset } from './ApprovalPresetControl.vue'
import { readRunSettings, writeRunSettings, type RunSettings } from './run-settings'
import { useVoiceInput } from '../../composables/useVoiceInput'
import ModelContextControl from './ModelContextControl.vue'
import CommandPalette, { type CommandEntry } from './CommandPalette.vue'
import { COMPOSER_COMMANDS, type ComposerCommand } from './composer-commands'
import { useI18n } from '../../i18n'
import ComposerTools from './ComposerTools.vue'
import type { ComposerAttachment } from './sketch'

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

const { t, locale } = useI18n()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  submit: [prompt: string]
  cancel: []
  'drop-files': [files: File[]]
  'update:approvalPreset': [value: ApprovalPreset]
  'run-command': [id: string]
  /** The model and effort the following runs should use. */
  'select-model': [selection: ModelSelectionRequestDto]
  'configure-model': []
  'retry-models': []
}>()

/**
 * The task's remembered settings. The composer reports every change upward and,
 * when it knows the task, also writes it under that task's key - a resumed run
 * has to keep the settings it was started with.
 */
const remembered = ref<RunSettings | null>(null)
const selectedApproval = ref<ApprovalPreset | null>(null)

watch(
  () => props.settingsKey,
  (key) => {
    selectedApproval.value = null
    remembered.value = key ? readRunSettings(key) : null
  },
  { immediate: true },
)

/**
 * The run the caller handed the composer outranks what the task remembered: the
 * caller is showing the settings of the run that is actually open, and a
 * remembered value is only a starting point for the next one.
 */
const approvalPreset = computed(() => selectedApproval.value ?? props.approvalPreset ?? remembered.value?.approvalPreset ?? 'ask')
watch(() => props.approvalPreset, () => { selectedApproval.value = null })

function updateApprovalPreset(value: ApprovalPreset): void {
  selectedApproval.value = value
  if (remembered.value) {
    remembered.value = { ...remembered.value, approvalPreset: value }
    if (props.settingsKey) writeRunSettings(props.settingsKey, remembered.value)
  }
  emit('update:approvalPreset', value)
}

const draft = ref(props.modelValue)
const fileInput = ref<HTMLInputElement | null>(null)
const folderInput = ref<HTMLInputElement | null>(null)
const attachments = ref<ComposerAttachment[]>([])
const toolsBusy = ref(false)
const attachmentError = ref(false)
const readingFiles = ref(false)
const voice = useVoiceInput(text => update(`${draft.value}${draft.value ? ' ' : ''}${text}`.slice(0, props.maxLength)), () => locale.value)
async function attachFiles(files: File[]): Promise<void> {
  if (props.disabled || readingFiles.value || files.length === 0) return
  emit('drop-files', files)
  attachmentError.value = false
  readingFiles.value = true
  try {
    const next: ComposerAttachment[] = []
    if (files.length + attachments.value.length > 20) throw new Error('too many attachments')
    for (const file of files) {
      if (file.size > 64 * 1024 || !/\.(txt|md|markdown|json|csv|log|yaml|yml|toml|js|ts|tsx|jsx|py|rs|css|html|vue|svg)$/i.test(file.name)) throw new Error('unsupported attachment')
      const text = await file.text()
      if (text.includes('\0')) throw new Error('binary attachment')
      const name = file.webkitRelativePath || file.name
      next.push({ id: crypto.randomUUID(), name, content: `${name}\n\`\`\`\n${text}\n\`\`\`` })
    }
    if ([...attachments.value, ...next].reduce((n, a) => n + a.content.length + 2, draft.value.length) > 28000) throw new Error('attachment too large')
    attachments.value.push(...next)
  } catch { attachmentError.value = true }
  finally { readingFiles.value = false; if (fileInput.value) fileInput.value.value = ''; if (folderInput.value) folderInput.value.value = '' }
}
function chooseFiles(event: Event): void { void attachFiles(Array.from((event.target as HTMLInputElement).files ?? [])) }
function attach(value: ComposerAttachment): boolean {
  if (props.disabled) return false
  if (attachments.value.length >= 20 || promptWithAttachments.value.length + value.content.length + 2 > 28000) { attachmentError.value = true; return false }
  attachmentError.value = false; attachments.value.push(value); return true
}
const promptWithAttachments = computed(() => [draft.value, ...attachments.value.map(a => a.content)].filter(Boolean).join('\n\n'))
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
  void attachFiles(Array.from(event.dataTransfer?.files ?? []))
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
 * While busy, the input accepts the next draft and offers both queue and stop.
 * The lifecycle also covers a start request that has not been acknowledged yet.
 */
const isBusy = computed(
  () =>
    props.busy ||
    props.lifecycle === 'submitting' ||
    props.lifecycle === 'running' ||
    props.lifecycle === 'cancelling',
)
watch(isBusy, busy => { if (busy) voice.stop() })

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
    (draft.value.trim().length > 0 || attachments.value.length > 0) &&
    draft.value.length <= props.maxLength &&
    promptWithAttachments.value.length <= 28000 &&
    !props.disabled && !readingFiles.value && !toolsBusy.value,
)

/**
 * The tally appears only once the limit is close.
 *
 * The reference's field carries no count at all, and a number that sits there
 * on every prompt is a row the reader pays for to learn something they only
 * need near the end. A tenth of the budget is the point where it starts to be
 * worth the row.
 */
const COUNT_VISIBLE_FROM = 0.8
const countVisible = computed(
  () => draft.value.length >= Math.floor(props.maxLength * COUNT_VISIBLE_FROM),
)

function update(value: string): void {
  draft.value = value
  commandsClosed.value = false
  emit('update:modelValue', value)
}

function submit(): void {
  if (!canSubmit.value) return
  const prompt = promptWithAttachments.value
  update('')
  attachments.value = []
  voice.stop()
  emit('submit', prompt)
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

/**
 * Put the caret in the field.
 *
 * The transcript's actions write into this composer, and an edit the reader has
 * to find the field for is an edit they did not make.
 */
const form = ref<HTMLFormElement | null>(null)

function focus(): void {
  form.value?.querySelector('textarea')?.focus()
}

defineExpose({ focus })
</script>

<template>
  <form
    ref="form"
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
    <CommandPalette
      :open="paletteOpen"
      :commands="paletteCommands"
      :query="draft"
      @select="emit('run-command', $event)"
      @close="commandsClosed = true"
    />
    <div class="run-composer__field" data-composer-field>
      <div v-if="attachments.length" class="run-composer__attachments" data-composer-attachments>
        <div v-for="attachment in attachments" :key="attachment.id" class="run-composer__attachment" :data-has-preview="!!attachment.preview">
          <img v-if="attachment.preview" :src="attachment.preview" :alt="attachment.name" />
          <FileText v-else :size="18" aria-hidden="true" /><span :title="attachment.name">{{ attachment.name }}</span>
          <button type="button" :aria-label="`${t('add.remove')} ${attachment.name}`" @click="attachments = attachments.filter(a => a.id !== attachment.id)"><X :size="14" /></button>
        </div>
      </div>
      <!-- The reference names the field with its placeholder and draws no
           heading over it; the accessible name is where that label went. -->
      <AppTextarea
        id="run-prompt"
        :model-value="draft"
        :aria-label="props.inputLabel ?? t('run.taskPrompt')"
        :placeholder="props.placeholder ?? t('composer.placeholder')"
        :max-length="props.maxLength"
        :disabled="props.disabled || readingFiles"
        :rows="rowCount"
        @update:model-value="update"
        @keydown="handleKeydown"
      />
      <div class="run-composer__footer" data-composer-footer>
        <span v-if="countVisible" class="run-composer__count" data-composer-count aria-live="polite">
          {{ draft.length }} / {{ props.maxLength }} {{ props.characterCountLabel ?? t('run.characters') }}
        </span>
        <div class="run-composer__leading">
          <input ref="fileInput" class="run-composer__file-input" type="file" multiple :aria-label="t('composer.attach')"
            accept=".txt,.md,.markdown,.json,.csv,.log,.yaml,.yml,.toml,.js,.ts,.tsx,.jsx,.py,.rs,.css,.html,.vue,.svg" @change="chooseFiles" />
          <input ref="folderInput" class="run-composer__file-input" type="file" multiple webkitdirectory :aria-label="t('add.chooseFolder')" @change="chooseFiles" />
          <ComposerTools :anchor="form" :workspace-name="props.workspaceName" :disabled="props.disabled || readingFiles" :attach-to-composer="attach"
            @files="folder => folder ? folderInput?.click() : fileInput?.click()" @busy="toolsBusy = $event" />
          <ApprovalPresetControl :model-value="approvalPreset" @update:model-value="updateApprovalPreset" />
        </div>
        <div class="run-composer__actions">
          <Spinner v-if="isBusy" data-run-activity class="run-composer__activity"
            :size="16" :label="props.activityLabel ?? t('run.runInProgress')" />
          <ModelContextControl
            class="run-composer__model"
            :catalog="props.modelCatalog"
            :status="props.modelStatus"
            @select="emit('select-model', $event)"
            @configure="emit('configure-model')"
            @retry="emit('retry-models')"
          />
          <button type="button" class="run-composer__icon" data-composer-mic :aria-pressed="voice.listening.value"
            :aria-label="t(voice.listening.value ? 'composer.stopVoice' : 'composer.voice')"
            :title="t(voice.supported ? 'composer.voice' : 'composer.voiceUnavailable')"
            :disabled="!voice.supported || props.disabled || isBusy || readingFiles" @click="voice.toggle()">
            <MicOff v-if="voice.listening.value" :size="21" aria-hidden="true" /><Mic v-else :size="21" aria-hidden="true" />
          </button>
          <AppButton
            v-if="isBusy"
            class="run-composer__send"
            type="button"
            variant="primary"
            data-composer-action="cancel"
            data-composer-action-shape="stop"
            :aria-label="props.cancelLabel ?? t('run.stop')"
            @click="emit('cancel')"
          >
            <Square :size="13" fill="currentColor" aria-hidden="true" />
          </AppButton>
          <AppButton
            v-if="!isBusy || draft.trim() || attachments.length"
            class="run-composer__send"
            type="submit"
            variant="primary"
            data-composer-action="submit"
            data-composer-action-shape="send"
            :disabled="!canSubmit"
            :aria-label="isBusy ? t('queue.send') : props.submitLabel ?? t('run.submit')"
          >
            <ArrowUp :size="16" aria-hidden="true" />
          </AppButton>
        </div>
      </div>
      <p v-if="attachmentError || voice.failed.value" class="run-composer__notice" role="status">{{ t(attachmentError ? 'composer.attachmentError' : 'composer.voiceError') }}</p>
    </div>
  </form>
</template>

<style scoped>
.run-composer {
  display: grid;
  gap: var(--space-2);
  min-inline-size: 0;
  flex-shrink: 0;
  max-inline-size: var(--composer-max-width, 54rem);
  margin-inline: auto;
}
.run-composer__attachments { display: flex; flex-wrap: wrap; gap: 8px; padding: 3px 4px 8px; }
.run-composer__attachment { display: flex; gap: 8px; align-items: center; max-width: 230px; padding: 8px 10px; border: 1px solid var(--color-border-base); border-radius: 12px; color: var(--color-text-secondary); background: var(--color-bg-element); font-size: 12px; position: relative; }
.run-composer__attachment > span { white-space: nowrap; text-overflow: ellipsis; overflow: hidden; }
.run-composer__attachment > svg { flex-shrink: 0; }
.run-composer__attachment button { display: grid; place-items: center; flex-shrink: 0; padding: 3px; border: 0; border-radius: 50%; background: var(--color-bg-element); color: var(--color-text-primary); cursor: pointer; }
.run-composer__attachment[data-has-preview='true'] { display: block; padding: 0; width: 160px; overflow: hidden; }
.run-composer__attachment img { display: block; width: 160px; height: 96px; object-fit: contain; background: white; }
.run-composer__attachment[data-has-preview='true'] > span { display: block; padding: 6px 10px; }
.run-composer__attachment[data-has-preview='true'] button { position: absolute; top: 5px; right: 5px; background: #303030; color: white; }

/* The field is the box, as the reference draws it: one rounded surface that
   holds the prompt and the controls under it, rather than a card with a second
   box inside it. */
.run-composer__field {
  display: grid;
  gap: var(--space-2);
  padding: calc(var(--space-4) - 2px) var(--space-3) calc(var(--space-2) + 2px);
  border: 1px solid var(--workspace-seam, var(--color-border-subtle));
  border-radius: var(--composer-radius, 1.25rem);
  background: var(--composer-surface, var(--color-bg-surface));
  box-shadow: none;
  transition: border-color var(--transition-fast) var(--ease-out), box-shadow var(--transition-fast) var(--ease-out);
}

.run-composer__field:focus-within {
  /* The boundary is the field's, not the prompt's: a ring inside the rounded
     frame would draw the second box this change removed. This is the
     replacement for the outline the prompt gives up below. */
  border-color: var(--color-border-control);
  box-shadow: 0 0 0 1px var(--color-border-control);
}

/* The drop target has to look like one: a drag that leaves the field
   unchanged reads as a drag the composer did not notice. */
.run-composer[data-composer-drag-active='true'] .run-composer__field {
  border-color: var(--color-accent);
  border-style: dashed;
  background: color-mix(in oklab, var(--color-accent) 6%, var(--color-bg-surface));
}

.run-composer__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.run-composer__commands {
  min-inline-size: 0;
  padding: 0 var(--space-4) var(--space-1);
}

/* The prompt sits in the field rather than in a box of its own: a second
   border inside the first is the box the reference does not draw. */
.run-composer :deep(.app-textarea) {
  min-block-size: 4rem;
  max-block-size: min(18rem, 30dvh);
  overflow-y: auto;
  padding: var(--space-2) var(--space-2) 0;
  border-color: transparent;
  background: transparent;
  border-radius: 0.75rem;
  box-shadow: none;
  /* The field grows with the prompt, so a resize grip is a handle for a job
     already done - and it sits where the send control is. */
  resize: none;
  font-size: 16px;
  line-height: 1.6;
}

.run-composer :deep(.app-textarea:focus-visible) {
  border-color: transparent;
  outline: none;
}

/* The action is a shape, as the reference's is: a round control that points the
   way the prompt goes, and a square one that stops the run. */
.run-composer__send {
  inline-size: 2.25rem;
  block-size: 2.25rem;
  min-inline-size: 2.25rem;
  padding: 0;
  border-radius: var(--radius-full);
  background: var(--composer-send);
  color: var(--composer-send-text);
  border: 0;
  transition: background var(--transition-fast) var(--ease-out), opacity var(--transition-fast) var(--ease-out);
}

.run-composer__leading { display: flex; align-items: center; gap: var(--space-2); min-inline-size: 0; }
.run-composer__file-input { display: none; }
.run-composer__icon { display: grid; place-items: center; width: 32px; height: 36px; flex: 0 0 32px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--color-text-primary); cursor: pointer; }
.run-composer__icon:hover { background: var(--workspace-hover); }
.run-composer__icon:disabled { opacity: .4; cursor: default; }
.run-composer__icon[aria-pressed='true'] { color: var(--composer-send); }
.run-composer__icon:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
.run-composer__notice { margin: 0 var(--space-2); font-size: 12px; color: var(--color-intent-warning-text); }

.run-composer__send :deep(.app-button__label) {
  display: inline-grid;
  place-items: center;
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
  min-inline-size: 0;
  margin-inline-start: auto;
}

/* The model control sits with the action it belongs to, as the reference draws
   it: what the next run uses, next to the control that starts it. */
.run-composer__model {
  min-inline-size: 0;
  margin-inline-end: var(--space-1);
}

.run-composer__activity {
  margin-inline-end: var(--space-1);
}

@media (max-width: 640px) {
  .run-composer {
    padding: 0;
  }

  .run-composer__field {
    padding-inline: var(--space-2);
  }

  .run-composer__footer {
    gap: var(--space-2);
  }

  .run-composer__actions {
    flex: 1;
    justify-content: flex-end;
  }

  .run-composer__model {
    flex: 0 1 auto;
    overflow: hidden;
  }

  .run-composer__send {
    flex: 0 0 2.25rem;
  }
}
</style>
