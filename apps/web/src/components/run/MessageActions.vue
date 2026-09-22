<script setup lang="ts">
/**
 * What a reader can do with a message they are looking at.
 *
 * One action, because it is the only one this product can answer: the text is
 * here, so it can be copied. The reference also offers regeneration and
 * feedback; Orchester's runtime has no way to re-run a turn and keeps no
 * feedback channel, and a button that does nothing is worse than a button that
 * is not there.
 *
 * The row is drawn on hover or focus, but it is never removed from the tree:
 * a control that only exists while a pointer is over it is a control a keyboard
 * user cannot reach.
 */
import { Check, Copy } from '@lucide/vue'
import { onScopeDispose, ref } from 'vue'

const props = defineProps<{
  text: string
  /** The accessible name of the copy control, resolved by the surface. */
  label: string
  /** What the control says it has done, announced and then retired. */
  copiedLabel: string
}>()

/** How long the control admits it copied before going back to offering to. */
const CONFIRMATION_MS = 2000

const copied = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null

async function copy(): Promise<void> {
  try {
    await navigator.clipboard?.writeText(props.text)
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
      <Check v-if="copied" :size="14" aria-hidden="true" />
      <Copy v-else :size="14" aria-hidden="true" />
    </button>
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