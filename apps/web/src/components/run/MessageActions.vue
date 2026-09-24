<script setup lang="ts">
/**
 * What a reader can do with a message they are looking at.
 *
 * Copying is always here, because the text is here. The reference's row is a few
 * controls in the open and a `…` for the rest, and that is the shape this draws:
 * copy, the one move that belongs to an answer - running its question again -
 * and a menu for whatever else the surface can answer.
 *
 * Two of the reference's own are still absent, and for the same reasons as
 * before: there is no feedback channel to carry a verdict, so no thumbs. The
 * third - regeneration - is here as "run this again" rather than under the
 * reference's word: this runtime starts a *new* run for the question instead of
 * replacing the turn in place, and a label that promised the second would be
 * promising something it does not do.
 *
 * The row is drawn on hover or focus, and stays up on the answer the reader is
 * looking at; it is never removed from the tree, because a control that only
 * exists while a pointer is over it is a control a keyboard user cannot reach.
 */
import { AppMenu, type AppMenuItem } from '@orchester/design'
import { Check, Copy, Ellipsis, RotateCcw } from '@lucide/vue'
import { computed, onScopeDispose, ref } from 'vue'

/**
 * An action the surface adds, named by the surface rather than here.
 *
 * The label is the whole of it: the row's menu is a list of words rather than
 * of glyphs, so there is no icon for a surface to choose.
 */
export interface MessageAction {
  id: string
  label: string
}

const props = withDefaults(
  defineProps<{
    text: string
    /** The accessible name of the copy control, resolved by the surface. */
    label: string
    /** What the control says it has done, announced and then retired. */
    copiedLabel: string
    /** Extra actions, drawn in the menu in the order they are given. */
    actions?: readonly MessageAction[]
    /**
     * The question to run again, when the surface has one for this row.
     *
     * An answer can be asked for again; a question cannot, which is why this is
     * a label to draw rather than a control always present.
     */
    rerunLabel?: string | undefined
    /** The menu's own accessible name. */
    moreLabel?: string
  }>(),
  { actions: () => [], moreLabel: '' },
)

const emit = defineEmits<{ action: [id: string] }>()

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

/** The surface's own actions, as the menu wants them. */
const menuItems = computed<AppMenuItem[]>(() =>
  props.actions.map((action) => ({ id: action.id, label: action.label })),
)

function runAgain(): void {
  emit('action', 'rerun')
}
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
      v-if="rerunLabel"
      class="message-actions__button"
      type="button"
      data-message-rerun
      :aria-label="rerunLabel"
      :title="rerunLabel"
      @click="runAgain"
    >
      <RotateCcw :size="14" aria-hidden="true" />
    </button>
    <AppMenu
      v-if="actions.length > 0"
      class="message-actions__more"
      align="end"
      :label="moreLabel"
      :items="menuItems"
      @select="emit('action', $event)"
    >
      <!--
        The menu draws its own trigger and puts this inside it, so the slot holds
        the glyph and nothing else: a button inside a button is not something a
        browser will let a reader press.
      -->
      <template #trigger>
        <Ellipsis :size="14" aria-hidden="true" data-message-more />
      </template>
    </AppMenu>
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

/* The menu draws the trigger; the row only says where it sits and how it looks. */
.message-actions__more :deep(.app-menu__trigger) {
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

.message-actions__more :deep(.app-menu__trigger:hover) {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}
</style>