<script setup lang="ts">
/**
 * The application shell.
 *
 * Section 2 of the design spec letters the shell's regions and names each one
 * with a data attribute, so the styles and the tests reach a region by what it
 * is rather than by which class moved recently. This component owns the three
 * columns - the rail, the transcript and the inspector - the two clamps the
 * columns are drawn at, and the drawer path that replaces them on a narrow
 * viewport.
 *
 * The regions keep the landmark each one already was: the attribute says which
 * region a box is, the element says what a reader should call it. The resize
 * handles are separators rather than buttons, because what they change is a
 * size and not a state, and they carry the arrow keys a separator is expected
 * to answer.
 */
import { AppButton, AppDrawer } from '@orchester/design'
import type { RailAppearance } from '@orchester/design'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import {
  INSPECTOR_MAX_WIDTH,
  INSPECTOR_MIN_WIDTH,
  INSPECTOR_PREFERRED_WIDTH,
  RAIL_MAX_WIDTH,
  RAIL_MIN_WIDTH,
  RAIL_PREFERRED_WIDTH,
  RESIZE_STEP,
  clampInspectorWidth,
  clampRailWidth,
  readShellWidths,
  writeShellWidths,
} from './shell-widths'

const props = withDefaults(
  defineProps<{
    sessionsTitle: string
    inspectorTitle: string
    controlsLabel?: string
    inspectorOpen?: boolean
    /** Solid or translucent, as the appearance axis asks. */
    railAppearance?: RailAppearance
    /** Whether the inspector has taken the whole width. */
    inspectorFullWidth?: boolean
    /** The inspector's active tab, for the styles that key off it. */
    inspectorTab?: string
    railResizeLabel?: string
    inspectorResizeLabel?: string
  }>(),
  {
    controlsLabel: 'Workspace panels',
    inspectorOpen: true,
    railAppearance: 'solid',
    inspectorFullWidth: false,
    inspectorTab: 'context',
    railResizeLabel: 'Resize the task rail',
    inspectorResizeLabel: 'Resize the inspector',
  },
)

const sessionsOpen = ref(false)
const inspectorDrawerOpen = ref(false)

/**
 * The two widths the user set, if any.
 *
 * Read on mount rather than at setup so the first paint is the default and a
 * stored preference lands on top of it, and written on every change rather
 * than on unload so a window closed mid-drag keeps where it was left.
 */
const railWidth = ref(RAIL_PREFERRED_WIDTH)
const inspectorWidth = ref(INSPECTOR_PREFERRED_WIDTH)

onMounted(() => {
  const stored = readShellWidths()
  const viewport = typeof window === 'undefined' ? RAIL_MAX_WIDTH + 360 : window.innerWidth
  if (stored.rail !== null) railWidth.value = clampRailWidth(stored.rail, viewport)
  if (stored.inspector !== null) inspectorWidth.value = clampInspectorWidth(stored.inspector)
})

function viewportWidth(): number {
  return typeof window === 'undefined' ? RAIL_MAX_WIDTH + 360 : window.innerWidth
}

function persist(): void {
  writeShellWidths({ rail: railWidth.value, inspector: inspectorWidth.value })
}

function setRailWidth(width: number): void {
  railWidth.value = clampRailWidth(width, viewportWidth())
  persist()
}

function setInspectorWidth(width: number): void {
  inspectorWidth.value = clampInspectorWidth(width)
  persist()
}

function handleRailKey(event: KeyboardEvent): void {
  if (event.key === 'ArrowLeft') setRailWidth(railWidth.value - RESIZE_STEP)
  else if (event.key === 'ArrowRight') setRailWidth(railWidth.value + RESIZE_STEP)
  else if (event.key === 'Home') setRailWidth(RAIL_MIN_WIDTH)
  else if (event.key === 'End') setRailWidth(RAIL_MAX_WIDTH)
  else return
  event.preventDefault()
}

function handleInspectorKey(event: KeyboardEvent): void {
  if (event.key === 'ArrowLeft') setInspectorWidth(inspectorWidth.value + RESIZE_STEP)
  else if (event.key === 'ArrowRight') setInspectorWidth(inspectorWidth.value - RESIZE_STEP)
  else if (event.key === 'Home') setInspectorWidth(INSPECTOR_MIN_WIDTH)
  else if (event.key === 'End') setInspectorWidth(INSPECTOR_MAX_WIDTH)
  else return
  event.preventDefault()
}

/**
 * Pointer drag, measured from the edge the handle sits on.
 *
 * The handle is the boundary rather than a grip inside a pane, so the rail
 * reads its width from the pointer's x and the inspector reads it from the
 * distance to the right edge. Both then go through the same clamp the keyboard
 * uses, because a pointer can travel past what the clamp allows.
 */
let dragging: 'rail' | 'inspector' | null = null

function startDrag(which: 'rail' | 'inspector', event: PointerEvent): void {
  dragging = which
  const target = event.currentTarget as HTMLElement | null
  target?.setPointerCapture?.(event.pointerId)
  if (which === 'rail') setRailWidth(event.clientX)
  else setInspectorWidth(viewportWidth() - event.clientX)
}

function moveDrag(event: PointerEvent): void {
  if (dragging === 'rail') setRailWidth(event.clientX)
  else if (dragging === 'inspector') setInspectorWidth(viewportWidth() - event.clientX)
}

function endDrag(): void {
  dragging = null
}

onBeforeUnmount(endDrag)
</script>

<template>
  <div class="app-shell" :class="{ 'app-shell--inspector-closed': !props.inspectorOpen }">
    <nav
      class="app-shell__mobile-controls"
      data-mobile-controls
      :aria-label="props.controlsLabel"
    >
      <AppButton
        variant="ghost"
        size="sm"
        data-mobile-sessions
        :aria-label="props.sessionsTitle"
        @click="sessionsOpen = true"
      >
        {{ props.sessionsTitle }}
      </AppButton>
      <AppButton
        variant="ghost"
        size="sm"
        data-mobile-inspector
        :aria-label="props.inspectorTitle"
        @click="inspectorDrawerOpen = true"
      >
        {{ props.inspectorTitle }}
      </AppButton>
    </nav>

    <div class="app-shell__grid">
      <nav
        class="app-shell__rail"
        data-pane="sessions"
        data-rail
        :data-rail-appearance="props.railAppearance"
        :data-rail-width="railWidth"
        :style="{ '--rail-width': railWidth + 'px' }"
        aria-label="Sessions"
      >
        <slot name="sessions" />
        <span
          class="app-shell__resize"
          data-rail-resize
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-label="props.railResizeLabel"
          :aria-valuenow="railWidth"
          :aria-valuemin="RAIL_MIN_WIDTH"
          :aria-valuemax="RAIL_MAX_WIDTH"
          @keydown="handleRailKey"
          @pointerdown.prevent="startDrag('rail', $event)"
          @pointermove="moveDrag"
          @pointerup="endDrag"
          @pointercancel="endDrag"
        />
      </nav>
      <main
        class="app-shell__transcript"
        data-pane="transcript"
        data-transcript
        aria-label="Run transcript"
      >
        <slot />
      </main>
      <aside
        class="app-shell__inspector"
        data-pane="inspector"
        data-inspector
        :data-inspector-open="String(props.inspectorOpen)"
        :data-inspector-full-width="String(props.inspectorFullWidth)"
        :data-inspector-tab="props.inspectorTab"
        :data-inspector-width="inspectorWidth"
        :style="{ '--inspector-width': inspectorWidth + 'px' }"
        :hidden="!props.inspectorOpen"
        aria-label="Inspector"
      >
        <span
          class="app-shell__resize"
          data-inspector-resize
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-label="props.inspectorResizeLabel"
          :aria-valuenow="inspectorWidth"
          :aria-valuemin="INSPECTOR_MIN_WIDTH"
          :aria-valuemax="INSPECTOR_MAX_WIDTH"
          @keydown="handleInspectorKey"
          @pointerdown.prevent="startDrag('inspector', $event)"
          @pointermove="moveDrag"
          @pointerup="endDrag"
          @pointercancel="endDrag"
        />
        <slot name="inspector" />
      </aside>
    </div>

    <AppDrawer v-model:open="sessionsOpen" :title="props.sessionsTitle" side="left">
      <slot name="sessions" />
    </AppDrawer>
    <AppDrawer v-model:open="inspectorDrawerOpen" :title="props.inspectorTitle" side="right">
      <slot name="inspector" />
    </AppDrawer>
  </div>
</template>

<style scoped>
.app-shell {
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
}

.app-shell__mobile-controls {
  display: none;
}

.app-shell__grid {
  display: grid;
  grid-template-columns:
    var(--rail-width, var(--rail-preferred-width, var(--sidebar-width)))
    minmax(0, 1fr)
    var(--inspector-width, var(--inspector-width-dynamic, var(--inspector-width)));
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  overflow: hidden;
}

.app-shell--inspector-closed .app-shell__grid {
  grid-template-columns:
    var(--rail-width, var(--rail-preferred-width, var(--sidebar-width)))
    minmax(0, 1fr);
}

.app-shell__rail,
.app-shell__inspector {
  position: relative;
}

.app-shell__rail,
.app-shell__transcript,
.app-shell__inspector {
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
}

.app-shell__rail,
.app-shell__inspector {
  background: var(--color-bg-surface);
}

.app-shell__rail {
  border-inline-end: 1px solid var(--color-border-base);
}

.app-shell__inspector {
  border-inline-start: 1px solid var(--color-border-base);
}

.app-shell__transcript {
  background: var(--color-bg-base);
}

/* The handle is the seam itself: a hairline the pointer can still find. */
.app-shell__resize {
  position: absolute;
  inset-block: 0;
  inline-size: 6px;
  cursor: col-resize;
  touch-action: none;
}

.app-shell__rail .app-shell__resize {
  inset-inline-end: -3px;
}

.app-shell__inspector .app-shell__resize {
  inset-inline-start: -3px;
}

.app-shell__resize:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

@media (max-width: 1279px) {
  .app-shell__mobile-controls {
    display: flex;
    min-block-size: var(--control-height-lg);
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-block-end: 1px solid var(--color-border-base);
    background: var(--color-bg-surface);
  }

  .app-shell__grid {
    grid-template-columns: minmax(0, 1fr);
    min-block-size: calc(
      100vh - var(--app-top-chrome-height, var(--header-height)) - var(--control-height-lg)
    );
  }

  .app-shell__rail,
  .app-shell__inspector {
    display: none;
  }
}
</style>
