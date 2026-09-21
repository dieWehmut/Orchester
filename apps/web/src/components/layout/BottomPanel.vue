<script setup lang="ts">
/**
 * The bottom panel, region I of the shell.
 *
 * Section 2 gives the panel three surfaces - a terminal, exec output and an
 * audit log - and makes it collapsible with its own tab mechanism, because a
 * tab that belongs to the panel is not a tab the window's tab strip owns. The
 * state and the chosen surface are the user's, so they are stored rather than
 * reset on every mount, and the height is clamped between the floor the spec
 * states and the share of the viewport it allows.
 *
 * The panel is a drawer under 900 px, which is the same breakpoint the columns
 * collapse at: a panel that needs 240 px of a phone's height is a panel that
 * would leave the transcript nothing.
 */
import { AppButton } from '@orchester/design'
import { computed, onMounted, ref } from 'vue'

import {
  BOTTOM_PANEL_DEFAULT_HEIGHT,
  BOTTOM_PANEL_MIN_HEIGHT,
  clampBottomPanelHeight,
  readBottomPanelState,
  writeBottomPanelState,
} from './bottom-panel-state'

const props = withDefaults(
  defineProps<{
    /** The panel's accessible name. */
    label: string
    /** The surfaces the panel can show, in the order they appear. */
    tabs: readonly { id: string; label: string }[]
    resizeLabel?: string
  }>(),
  { resizeLabel: 'Resize the bottom panel' },
)

const expanded = ref(false)
const activeTab = ref(props.tabs[0]?.id ?? 'terminal')
const height = ref(BOTTOM_PANEL_DEFAULT_HEIGHT)

function viewportHeight(): number {
  return typeof window === 'undefined' ? BOTTOM_PANEL_DEFAULT_HEIGHT : window.innerHeight
}

onMounted(() => {
  const stored = readBottomPanelState()
  expanded.value = stored.expanded
  // A stored surface the panel no longer offers is not a surface.
  if (props.tabs.some((tab) => tab.id === stored.tab)) activeTab.value = stored.tab
})

function persist(): void {
  writeBottomPanelState({ expanded: expanded.value, tab: activeTab.value })
}

function setExpanded(next: boolean): void {
  expanded.value = next
  persist()
}

function selectTab(id: string): void {
  activeTab.value = id
  persist()
}

function setHeight(next: number): void {
  height.value = clampBottomPanelHeight(next, viewportHeight())
}

function handleResizeKey(event: KeyboardEvent): void {
  if (event.key === 'ArrowUp') setHeight(height.value + 16)
  else if (event.key === 'ArrowDown') setHeight(height.value - 16)
  else if (event.key === 'Home') setHeight(BOTTOM_PANEL_MIN_HEIGHT)
  else if (event.key === 'End') setHeight(viewportHeight())
  else return
  event.preventDefault()
}

const maxHeight = computed(() => clampBottomPanelHeight(viewportHeight(), viewportHeight()))

let dragging = false

function startDrag(event: PointerEvent): void {
  dragging = true
  ;(event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId)
}

function moveDrag(event: PointerEvent): void {
  if (!dragging) return
  // The handle sits on the panel's top edge, so the height is the distance
  // from the pointer down to the bottom of the window.
  setHeight(viewportHeight() - event.clientY)
}

function endDrag(): void {
  dragging = false
}
</script>

<template>
  <section
    class="bottom-panel"
    data-bottom-panel
    :data-bottom-panel-state="expanded ? 'expanded' : 'collapsed'"
    :data-bottom-panel-height="height"
    :aria-label="props.label"
  >
    <header class="bottom-panel__bar" data-bottom-panel-bar>
      <AppButton
        variant="ghost"
        size="sm"
        data-bottom-panel-toggle
        :aria-label="props.label"
        :aria-expanded="expanded ? 'true' : 'false'"
        @click="setExpanded(!expanded)"
      >
        {{ props.label }}
      </AppButton>
      <div
        v-if="expanded"
        class="bottom-panel__tabs"
        role="tablist"
        data-bottom-panel-tabs
      >
        <button
          v-for="tab in props.tabs"
          :key="tab.id"
          class="bottom-panel__tab"
          type="button"
          role="tab"
          :data-bottom-panel-tab="tab.id"
          :aria-selected="tab.id === activeTab ? 'true' : 'false'"
          @click="selectTab(tab.id)"
        >
          {{ tab.label }}
        </button>
      </div>
    </header>

    <span
      v-if="expanded"
      class="bottom-panel__resize"
      data-bottom-panel-resize
      role="separator"
      tabindex="0"
      aria-orientation="horizontal"
      :aria-label="props.resizeLabel"
      :aria-valuenow="height"
      :aria-valuemin="BOTTOM_PANEL_MIN_HEIGHT"
      :aria-valuemax="maxHeight"
      @keydown="handleResizeKey"
      @pointerdown.prevent="startDrag"
      @pointermove="moveDrag"
      @pointerup="endDrag"
      @pointercancel="endDrag"
    />

    <div
      v-if="expanded"
      class="bottom-panel__body"
      data-bottom-panel-body
      :style="{ '--bottom-panel-height': height + 'px' }"
    >
      <div role="tabpanel" :data-bottom-panel-surface="activeTab">
        <slot :name="activeTab" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.bottom-panel {
  position: relative;
  border-block-start: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
}

.bottom-panel__bar {
  display: flex;
  min-block-size: var(--control-height-lg, 32px);
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-2);
}

.bottom-panel__tabs {
  display: flex;
  gap: var(--space-1);
}

.bottom-panel__tab {
  min-block-size: var(--hit-target-min, 32px);
  padding-inline: var(--space-2);
  border: 0;
  border-radius: var(--radius-xs);
  background: none;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}

.bottom-panel__tab[aria-selected='true'] {
  background: var(--color-accent-muted);
  color: var(--color-text-primary);
}

.bottom-panel__resize {
  position: absolute;
  inset-block-start: -3px;
  inset-inline: 0;
  block-size: 6px;
  cursor: row-resize;
  touch-action: none;
}

.bottom-panel__resize:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.bottom-panel__body {
  block-size: var(--bottom-panel-height);
  max-block-size: var(--bottom-panel-max-height, 70vh);
  overflow: auto;
}
</style>
