<script setup lang="ts">
/**
 * The application shell.
 *
 * Section 2 of the design spec letters the shell's regions and names each one
 * with a data attribute, so the styles and the tests reach a region by what it
 * is rather than by which class moved recently. The shell draws **two** regions
 * - the rail and the transcript - and the drawer that replaces the rail on a
 * narrow viewport.
 *
 * There is no right column. The reference the product is built against has none
 * - its conversation runs to the window's edge - and the surfaces that used to
 * live there (the run's context, its approvals and the working copy's review)
 * are drawn in the bottom panel, which is the shell's remaining collapsible
 * region and already owns a tab mechanism of its own.
 *
 * The regions keep the landmark each one already was: the attribute says which
 * region a box is, the element says what a reader should call it. The resize
 * handle is a separator rather than a button, because what it changes is a size
 * and not a state, and it carries the arrow keys a separator is expected to
 * answer.
 */
import { AppButton, AppDrawer } from '@orchester/design'
import type { RailAppearance } from '@orchester/design'
import { onBeforeUnmount, onMounted, onUnmounted, ref } from 'vue'

import { useI18n } from '../../i18n'
import { useRailCollapsed } from '../../composables/use-rail-collapsed'
import { useShortcut } from '../../shortcuts'
import {
  registerShellAction,
  registerShellActionExpanded,
} from './shell-actions'

import {
  RAIL_MAX_WIDTH,
  RAIL_MIN_WIDTH,
  RAIL_PREFERRED_WIDTH,
  RESIZE_STEP,
  clampRailWidth,
  readShellWidths,
  writeShellWidths,
} from './shell-widths'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    sessionsTitle: string
    controlsLabel?: string
    /** Solid or translucent, as the appearance axis asks. */
    railAppearance?: RailAppearance
    railResizeLabel?: string
  }>(),
  {
    railAppearance: 'solid',
  },
)

const { collapsed: railCollapsed, toggle: toggleRail } = useRailCollapsed()

const sessionsOpen = ref(false)

/**
 * Below this width the rail is a drawer rather than a column, section 2.1.
 *
 * The row's toggle has to answer both, so the shell tracks which of the two it
 * is drawing: at a narrow viewport a fold that hid the column would hide
 * nothing, and the drawer is what the reader would be looking at.
 */
const NARROW_VIEWPORT_PX = 1280
const narrowViewport = ref(false)

function readViewport(): void {
  narrowViewport.value = typeof window !== 'undefined' && window.innerWidth < NARROW_VIEWPORT_PX
}

/**
 * The rail's two entry points, registered for as long as the shell is mounted.
 *
 * The action reaches whichever rail is on screen, and the state tells the row
 * which way the control goes: the drawer is closed by default and the column is
 * open by default, so "collapsed" is a different question in each face.
 */
const unregisterToggle = registerShellAction('rail.toggle', () => {
  if (narrowViewport.value) sessionsOpen.value = !sessionsOpen.value
  else toggleRail()
})
const unregisterExpanded = registerShellActionExpanded('rail.toggle', () =>
  narrowViewport.value ? sessionsOpen.value : !railCollapsed.value,
)

// The chord the design spec gives the collapse, bound where the rail is drawn.
useShortcut(
  {
    id: 'rail.toggle',
    labelKey: 'shortcuts.labels.railToggle',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', 'B'],
  },
  () => {
    if (narrowViewport.value) sessionsOpen.value = !sessionsOpen.value
    else toggleRail()
  },
)

onUnmounted(() => {
  unregisterToggle()
  unregisterExpanded()
})

onMounted(readViewport)
onBeforeUnmount(() => {
  if (typeof window === 'undefined') return
  window.removeEventListener('resize', readViewport)
})

if (typeof window !== 'undefined') {
  onMounted(() => window.addEventListener('resize', readViewport))
}
/**
 * The width the user set, if any.
 *
 * Read on mount rather than at setup so the first paint is the default and a
 * stored preference lands on top of it, and written on every change rather than
 * on unload so a window closed mid-drag keeps where it was left.
 */
const railWidth = ref(RAIL_PREFERRED_WIDTH)

onMounted(() => {
  const stored = readShellWidths()
  const viewport = typeof window === 'undefined' ? RAIL_MAX_WIDTH + 360 : window.innerWidth
  if (stored.rail !== null) railWidth.value = clampRailWidth(stored.rail, viewport)
})

function viewportWidth(): number {
  return typeof window === 'undefined' ? RAIL_MAX_WIDTH + 360 : window.innerWidth
}

function persist(): void {
  writeShellWidths({ rail: railWidth.value })
}

function setRailWidth(width: number): void {
  railWidth.value = clampRailWidth(width, viewportWidth())
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

/**
 * Pointer drag, measured from the edge the handle sits on: the rail reads its
 * width from the pointer's x, through the same clamp the keyboard uses, because
 * a pointer can travel past what the clamp allows.
 */
let dragging = false

function startDrag(event: PointerEvent): void {
  dragging = true
  ;(event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId)
  setRailWidth(event.clientX)
}

function moveDrag(event: PointerEvent): void {
  if (dragging) setRailWidth(event.clientX)
}

function endDrag(): void {
  dragging = false
}

onBeforeUnmount(endDrag)
</script>

<template>
  <div
    class="app-shell"
    :class="{ 'app-shell--rail-closed': railCollapsed }"
    :data-rail-folded="String(railCollapsed)"
  >
    <nav
      class="app-shell__mobile-controls"
      data-mobile-controls
      :aria-label="props.controlsLabel ?? t('layout.workspacePanels')"
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
    </nav>

    <div class="app-shell__grid">
      <nav
        v-if="!railCollapsed"
        class="app-shell__rail"
        data-pane="sessions"
        data-rail
        :data-rail-appearance="props.railAppearance"
        :data-rail-width="railWidth"
        :style="{ '--rail-width': railWidth + 'px' }"
        :aria-label="t('layout.sessions')"
      >
        <slot name="sessions" />
        <span
          class="app-shell__resize"
          data-rail-resize
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-label="props.railResizeLabel ?? t('layout.resizeRail')"
          :aria-valuenow="railWidth"
          :aria-valuemin="RAIL_MIN_WIDTH"
          :aria-valuemax="RAIL_MAX_WIDTH"
          @keydown="handleRailKey"
          @pointerdown.prevent="startDrag($event)"
          @pointermove="moveDrag"
          @pointerup="endDrag"
          @pointercancel="endDrag"
        />
      </nav>
      <main
        class="app-shell__transcript"
        data-pane="transcript"
        data-transcript
        :aria-label="t('layout.transcript')"
      >
        <slot />
      </main>
    </div>

    <AppDrawer v-model:open="sessionsOpen" :title="props.sessionsTitle" side="left">
      <slot name="sessions" />
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
    minmax(0, 1fr);
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  overflow: hidden;
}

.app-shell__rail {
  position: relative;
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
  background: var(--color-bg-surface);
  border-inline-end: 1px solid var(--color-border-base);
}

.app-shell__transcript {
  min-inline-size: 0;
  min-block-size: 0;
  overflow: auto;
  background: var(--color-bg-base);
}

/* The handle is the seam itself: a hairline the pointer can still find. */
.app-shell__resize {
  position: absolute;
  inset-block: 0;
  inset-inline-end: -3px;
  inline-size: 6px;
  cursor: col-resize;
  touch-action: none;
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

  .app-shell__rail {
    display: none;
  }
}
</style>
