<script setup lang="ts">
/**
 * The unified tab strip, region B of the shell.
 *
 * Section 4.2 makes this the one strip for open tasks, terminals, diffs and
 * agents: a tab names what it opens rather than which pane drew it, which is
 * what lets the strip survive a pane being replaced. The strip is a controller
 * rather than an owner - the list and the selection are the shell's, and this
 * component reports the intent to change them - because a tab that closed
 * itself would be a tab the shell could not refuse to close.
 *
 * The overflow attribute is measured rather than guessed: the strip scrolls,
 * and whether it overflows is a fact about its own box.
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'

import { cycleTab, tabAt, type ShellTab } from './tab-strip'

const props = withDefaults(
  defineProps<{
    tabs: readonly ShellTab[]
    activeId: string
    label: string
  }>(),
  {},
)

const emit = defineEmits<{
  (event: 'select', id: string): void
  (event: 'close', id: string): void
  (event: 'reorder', move: { from: string; to: string }): void
}>()

const list = ref<HTMLElement | null>(null)
const overflowing = ref(false)
let observer: ResizeObserver | null = null

/** A strip overflows when its content is wider than the box it sits in. */
function measure(): void {
  const element = list.value
  if (!element) return
  const scrollWidth = element.scrollWidth ?? 0
  const clientWidth = element.clientWidth ?? 0
  overflowing.value = scrollWidth > clientWidth
}

onMounted(() => {
  measure()
  if (typeof ResizeObserver === 'function' && list.value) {
    observer = new ResizeObserver(measure)
    observer.observe(list.value)
  }
})

onUnmounted(() => {
  observer?.disconnect()
  observer = null
})

/**
 * The chords the strip answers.
 *
 * They are read from the strip's own listener rather than from a global one
 * because a chord that changes tabs should only do so while the strip is the
 * thing under the pointer or the focus, and because the shell's registry owns
 * the chords that belong to the whole window.
 */
function handleKey(event: KeyboardEvent): void {
  const mod = event.ctrlKey || event.metaKey
  if (!mod) return

  if (event.key === 'Tab') {
    const next = cycleTab(props.tabs, props.activeId, event.shiftKey)
    if (next !== null) emit('select', next)
    event.preventDefault()
    return
  }

  if (event.key === 'w') {
    emit('close', props.activeId)
    event.preventDefault()
    return
  }

  const position = Number(event.key)
  if (Number.isInteger(position) && position >= 1 && position <= 9) {
    const target = tabAt(props.tabs, position)
    if (target !== null) emit('select', target)
    event.preventDefault()
  }
}

let dragging: string | null = null

function startDrag(id: string): void {
  dragging = id
}

function dropOn(id: string): void {
  if (dragging === null || dragging === id) {
    dragging = null
    return
  }
  emit('reorder', { from: dragging, to: id })
  dragging = null
}

const listStyle = computed(() => ({ '--tabstrip-height': 'var(--tabstrip-height, 36px)' }))
</script>

<template>
  <div
    class="tab-strip"
    data-tabstrip
    :data-tabstrip-overflow="String(overflowing)"
    :style="listStyle"
  >
    <div
      ref="list"
      class="tab-strip__list"
      role="tablist"
      :aria-label="props.label"
      @keydown="handleKey"
      @scroll="measure"
    >
      <div
        v-for="tab in props.tabs"
        :key="tab.id"
        class="tab-strip__tab"
        role="tab"
        tabindex="0"
        :data-tabstrip-tab="tab.id"
        :data-tabstrip-kind="tab.kind"
        :aria-selected="tab.id === props.activeId ? 'true' : 'false'"
        draggable="true"
        @click="emit('select', tab.id)"
        @auxclick.middle.prevent="emit('close', tab.id)"
        @dragstart="startDrag(tab.id)"
        @dragover.prevent
        @drop="dropOn(tab.id)"
      >
        <span class="tab-strip__label" data-tabstrip-label>{{ tab.label }}</span>
        <button
          class="tab-strip__close"
          type="button"
          data-tabstrip-close
          :aria-label="`Close ${tab.label}`"
          @click.stop="emit('close', tab.id)"
        >
          ×
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tab-strip {
  min-block-size: var(--tabstrip-height, 36px);
  border-block-end: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
}

.tab-strip__list {
  display: flex;
  min-block-size: var(--tabstrip-height, 36px);
  align-items: stretch;
  gap: var(--space-1);
  padding-inline: var(--space-2);
  overflow-x: auto;
  scrollbar-width: thin;
}

/* The fades follow the measured overflow rather than the tab count, so a strip
   that fits draws no fade at all. */
.tab-strip[data-tabstrip-overflow='true'] .tab-strip__list {
  mask-image: linear-gradient(
    to right,
    transparent 0,
    black var(--space-3),
    black calc(100% - var(--space-3)),
    transparent 100%
  );
}

.tab-strip__tab {
  display: inline-flex;
  max-inline-size: 16rem;
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-3);
  border-block-end: 2px solid transparent;
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
  white-space: nowrap;
  cursor: pointer;
}

.tab-strip__tab[aria-selected='true'] {
  border-block-end-color: var(--color-accent);
  background: var(--color-surface-base);
  color: var(--color-text-primary);
}

.tab-strip__label {
  overflow: hidden;
  text-overflow: ellipsis;
}

.tab-strip__close {
  display: inline-flex;
  min-inline-size: var(--hit-target-min, 32px);
  min-block-size: var(--hit-target-min, 32px);
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: var(--radius-xs);
  background: none;
  color: inherit;
  cursor: pointer;
}

.tab-strip__close:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}
</style>
