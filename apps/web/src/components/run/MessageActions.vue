<script setup lang="ts">
/**
 * What a reader can do with a message they are looking at.
 *
 * Copy, speech and Markdown export operate on the displayed text. Actions that
 * change the conversation are emitted to the surface that owns the run.
 */
import { Check, Copy, Pencil, Quote, RotateCcw, Volume2, Square, Download } from '@lucide/vue'
import { onScopeDispose, ref } from 'vue'
import { useI18n } from '../../i18n'

/** An action the surface adds, named by the surface rather than here. */
export interface MessageAction {
  id: string
  label: string
  icon: 'quote' | 'reuse' | 'edit'
}

const props = withDefaults(
  defineProps<{
    text: string
    extended?: boolean
    /** The accessible name of the copy control, resolved by the surface. */
    label: string
    /** What the control says it has done, announced and then retired. */
    copiedLabel: string
    /** Extra actions, drawn after the copy in the order they are given. */
    actions?: readonly MessageAction[]
  }>(),
  { actions: () => [] },
)

const emit = defineEmits<{ action: [id: string] }>()
const { t, locale } = useI18n()

const GLYPHS = { quote: Quote, reuse: RotateCcw, edit: Pencil } as const

/** How long the control admits it copied before going back to offering to. */
const CONFIRMATION_MS = 2000

const speaking = ref(false)
const speechAvailable = typeof window !== 'undefined' && 'speechSynthesis' in window
function speak() {
  if (!speechAvailable) return
  if (speaking.value) { window.speechSynthesis.cancel(); speaking.value = false; return }
  const utterance = new SpeechSynthesisUtterance(props.text)
  utterance.lang = locale.value
  utterance.onend = utterance.onerror = () => { speaking.value = false }
  window.speechSynthesis.cancel(); window.speechSynthesis.speak(utterance); speaking.value = true
}
function download() {
  const url = URL.createObjectURL(new Blob([props.text], { type: 'text/markdown;charset=utf-8' }))
  const link = document.createElement('a'); link.href = url; link.download = 'orchester-message.md'; link.click()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
const copied = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null

async function copy(): Promise<void> {
  try {
    if (!navigator.clipboard) return
    await navigator.clipboard.writeText(props.text)
  } catch {
    // A clipboard the browser refuses is not worth an error banner over.
    return
  }
  copied.value = true
  if (timer !== null) clearTimeout(timer)
  timer = setTimeout(() => {
    copied.value = false
    timer = null
  }, CONFIRMATION_MS)
}

onScopeDispose(() => {
  if (timer !== null) clearTimeout(timer)
  if (speaking.value && speechAvailable) window.speechSynthesis.cancel()
})
</script>

<template>
  <div class="message-actions" data-message-actions>
    <button
      class="message-actions__button"
      type="button"
      data-message-copy
      :data-copied="copied ? 'true' : 'false'"
      :aria-label="label"
      :title="label"
      @click="copy"
    >
      <Check v-if="copied" :size="16" aria-hidden="true" />
      <Copy v-else :size="16" aria-hidden="true" />
    </button>
    <button
      v-for="action in actions"
      :key="action.id"
      class="message-actions__button"
      type="button"
      :data-message-action="action.id"
      :aria-label="action.label"
      :title="action.label"
      @click="emit('action', action.id)"
    >
      <component :is="GLYPHS[action.icon]" :size="16" aria-hidden="true" />
    </button>
    <button v-if="extended && speechAvailable" class="message-actions__button" type="button" data-message-speech :aria-label="speaking ? t('transcript.stopReading') : t('transcript.readAloud')" :title="speaking ? t('transcript.stopReading') : t('transcript.readAloud')" :aria-pressed="speaking" @click="speak"><Square v-if="speaking" :size="16" aria-hidden="true" /><Volume2 v-else :size="16" aria-hidden="true" /></button>
    <button v-if="extended" class="message-actions__button" type="button" data-message-download :aria-label="t('transcript.downloadMarkdown')" :title="t('transcript.downloadMarkdown')" @click="download"><Download :size="16" aria-hidden="true" /></button>
    <span class="message-actions__status" data-message-copy-status role="status" aria-live="polite">
      {{ copied ? copiedLabel : '' }}
    </span>
  </div>
</template>

<style scoped>
.message-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.message-actions__button {
  display: inline-grid;
  min-inline-size: var(--hit-target-min, 32px);
  min-block-size: var(--hit-target-min, 32px);
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  place-items: center;
}

.message-actions__button:hover {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.message-actions__button:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.message-actions__status {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}
</style>
