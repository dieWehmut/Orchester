<script setup lang="ts">
/**
 * What a reader can do with a message they are looking at.
 *
 * Copying is always here, because the text is here. Everything else arrives from
 * the surface, with its own words and its own glyph: the reference hangs a row
 * of icons off a message, and the row is only worth its space if each control
 * does something this product can answer. Regeneration and feedback are the
 * reference's other two, and neither is offered - the runtime cannot re-run a
 * turn in place and keeps no feedback channel - so what a surface can add here
 * is the reader's next move rather than the agent's.
 *
 * The row is drawn on hover or focus, but it is never removed from the tree:
 * a control that only exists while a pointer is over it is a control a keyboard
 * user cannot reach.
 */
import { Check, Copy, Quote, RotateCcw } from '@lucide/vue'
import { onScopeDispose, ref } from 'vue'

/** An action the surface adds, named by the surface rather than here. */
export interface MessageAction {
  id: string
  label: string
  icon: 'quote' | 'reuse'
}

const props = withDefaults(
  defineProps<{
    text: string
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

const GLYPHS = { quote: Quote, reuse: RotateCcw } as const

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
      <component :is="GLYPHS[action.icon]" :size="14" aria-hidden="true" />
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