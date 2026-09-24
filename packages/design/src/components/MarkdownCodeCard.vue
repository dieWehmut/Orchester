<script setup lang="ts">
/**
 * A block of code, drawn as the reference draws it: a card whose head names the
 * language it was written in, with the two things a reader does to a block -
 * wrap it, or take it away.
 *
 * The head is not decoration. A fence with no language is a block of text an
 * agent thought was worth separating; naming that "plain text" says what it is,
 * and naming `powershell` says which interpreter the reader is about to paste
 * into. Wrapping is a property of *this* block rather than of the page, because
 * the block a reader wants wrapped is the one whose line is running off the
 * edge, not every block in the transcript.
 */
import { computed, ref } from 'vue'

import IconButton from './IconButton.vue'

const props = withDefaults(
  defineProps<{
    text: string
    /** The fence's info string, when it had one. */
    language?: string | null
    /** What to call a block that named no language; the surface owns the words. */
    plainLabel?: string
    wrapLabel?: string
    unwrapLabel?: string
    copyLabel?: string
    copiedLabel?: string
  }>(),
  {
    language: null,
    plainLabel: '',
    wrapLabel: '',
    unwrapLabel: '',
    copyLabel: '',
    copiedLabel: '',
  },
)

/** Recorded here rather than by the transcript: wrapping one block wraps one. */
const wrapped = ref(false)
const copied = ref(false)

const label = computed(() => {
  const language = props.language?.trim() ?? ''
  return language.length > 0 ? language : props.plainLabel
})

async function copy(): Promise<void> {
  if (typeof navigator === 'undefined' || navigator.clipboard === undefined) return
  try {
    await navigator.clipboard.writeText(props.text)
    copied.value = true
    // The word goes back to "copy" after a moment, so the control keeps saying
    // what pressing it does.
    setTimeout(() => {
      copied.value = false
    }, 1600)
  } catch {
    // A refused clipboard is not a reason to change what the block says; the
    // reader can still select the text they can see.
  }
}
</script>

<template>
  <div
    class="code-card"
    data-markdown-code
    data-code-card
    :data-code-wrap="wrapped ? 'true' : 'false'"
    :data-code-language="language ?? ''"
  >
    <div class="code-card__head" data-code-head>
      <span class="code-card__mark" aria-hidden="true">&lt;/&gt;</span>
      <span class="code-card__language" data-code-language-label>{{ label }}</span>
      <span class="code-card__actions">
        <IconButton
          :label="wrapped ? unwrapLabel : wrapLabel"
          :active="wrapped"
          data-code-wrap-toggle
          @click="wrapped = !wrapped"
        >
          <svg
            class="code-card__icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M3 10h18M3 14h12" />
          </svg>
        </IconButton>
        <IconButton
          :label="copied ? copiedLabel : copyLabel"
          :active="copied"
          data-code-copy
          @click="copy"
        >
          <svg
            class="code-card__icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <rect x="9" y="9" width="12" height="12" rx="2" />
            <path d="M5 15V5a2 2 0 0 1 2-2h10" />
          </svg>
        </IconButton>
      </span>
    </div>
    <pre class="code-card__body" data-code-body><code>{{ text }}</code></pre>
  </div>
</template>

<style scoped>
.code-card {
  display: grid;
  overflow: hidden;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
}

.code-card__head {
  display: flex;
  min-block-size: var(--control-height-sm, 28px);
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-2);
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
}

.code-card__mark {
  font-family: var(--font-mono);
  opacity: 0.7;
}

.code-card__language {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* The actions sit at the far end, as the reference seats them. */
.code-card__actions {
  display: flex;
  margin-inline-start: auto;
  align-items: center;
  gap: var(--space-1);
}

.code-card__icon {
  inline-size: 15px;
  block-size: 15px;
}

.code-card__body {
  overflow: auto;
  margin: 0;
  padding: var(--space-3);
  border-block-start: 1px solid var(--color-border-base);
  background: var(--color-bg-base);
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: 1.6;
  tab-size: 2;
  white-space: pre;
}

/* Wrapping is this block's own answer to a long line, not the page's. */
.code-card[data-code-wrap='true'] .code-card__body {
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
</style>