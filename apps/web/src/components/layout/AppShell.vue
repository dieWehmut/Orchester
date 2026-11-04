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
import { computed, onBeforeUnmount, onMounted, onUnmounted, ref, useSlots, watch } from 'vue'

import { useI18n } from '../../i18n'
import { useRailCollapsed } from '../../composables/use-rail-collapsed'
import { useShortcut } from '../../shortcuts'
import {
  registerShellAction,
  registerShellActionExpanded,
} from './shell-actions'

import {
  INSPECTOR_MAX_WIDTH,
  INSPECTOR_MIN_WIDTH,
  INSPECTOR_PREFERRED_WIDTH,
  RAIL_MAX_WIDTH,
  RAIL_MIN_WIDTH,
  RAIL_PREFERRED_WIDTH,
  TRANSCRIPT_MIN_WIDTH,
  RESIZE_STEP,
  clampInspectorWidth,
  clampRailWidth,
  readShellWidths,
  writeShellWidths,
} from './shell-widths'

const { t } = useI18n()
const slots = useSlots()

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
    /** True when the routed view has moved the destinations into its own rail. */
    navigationInRail?: boolean
  }>(),
  {
    inspectorOpen: true,
    railAppearance: 'solid',
    inspectorFullWidth: false,
    inspectorTab: 'context',
    navigationInRail: false,
  },
)

const { collapsed: railCollapsed, toggle: toggleRail } = useRailCollapsed()
const emit = defineEmits<{ 'update:inspectorOpen': [open: boolean] }>()

const sessionsOpen = ref(false)
defineExpose({ closeSessions: () => { sessionsOpen.value = false } })

/**
 * Below this width the rail is a drawer rather than a column, section 2.1.
 *
 * The row's toggle has to answer both, so the shell tracks which of the two it
 * is drawing: at a narrow viewport a fold that hid the column would hide
 * nothing, and the drawer is what the reader would be looking at.
 */
const NARROW_VIEWPORT_PX = 800
const INSPECTOR_DRAWER_PX = 1120
const shellElement = ref<HTMLElement | null>(null)
let shellObserver: ResizeObserver | undefined
const narrowViewport = ref(false)
const inspectorDocked = ref(true)
const availableWidth = ref(typeof window === 'undefined' ? 1280 : window.innerWidth)
const navigationWidth = computed(() =>
  slots.navigation && !narrowViewport.value && !props.navigationInRail ? 52 : 0,
)
const contentWidth = computed(() => availableWidth.value - navigationWidth.value - 8)
const inspectorDrawerOpen = ref(false)

function readViewport(): void {
  if (typeof window === 'undefined') return
  availableWidth.value = shellElement.value?.getBoundingClientRect().width || window.innerWidth
  // CSS media queries use the viewport; panel widths use the actual outlet.
  narrowViewport.value = window.innerWidth < NARROW_VIEWPORT_PX
  inspectorDocked.value = window.innerWidth >= INSPECTOR_DRAWER_PX
  if (!narrowViewport.value) sessionsOpen.value = false
  inspectorDrawerOpen.value = !inspectorDocked.value && props.inspectorOpen
}

watch(() => props.inspectorOpen, (open) => {
  inspectorDrawerOpen.value = !inspectorDocked.value && open
  if (open) sessionsOpen.value = false
})

function openSessions(): void {
  emit('update:inspectorOpen', false)
  inspectorDrawerOpen.value = false
  sessionsOpen.value = true
}

function updateInspectorDrawer(open: boolean): void {
  inspectorDrawerOpen.value = open
  emit('update:inspectorOpen', open)
  if (open) sessionsOpen.value = false
}

function toggleSessions(): void {
  if (!narrowViewport.value) toggleRail()
  else if (sessionsOpen.value) sessionsOpen.value = false
  else openSessions()
}

/**
 * The rail's two entry points, registered for as long as the shell is mounted.
 *
 * The action reaches whichever rail is on screen, and the state tells the row
 * which way the control goes: the drawer is closed by default and the column is
 * open by default, so "collapsed" is a different question in each face.
 */
const unregisterToggle = registerShellAction('rail.toggle', () => {
  toggleSessions()
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
    toggleSessions()
  },
)

onUnmounted(() => {
  unregisterToggle()
  unregisterExpanded()
})

onMounted(() => {
  readViewport()
  if (typeof ResizeObserver !== 'undefined' && shellElement.value) {
    shellObserver = new ResizeObserver(readViewport)
    shellObserver.observe(shellElement.value)
  }
})
onBeforeUnmount(() => {
  shellObserver?.disconnect()
  if (typeof window === 'undefined') return
  window.removeEventListener('resize', readViewport)
})

if (typeof window !== 'undefined') {
  onMounted(() => window.addEventListener('resize', readViewport))
}

/**
 * The two widths the user set, if any.
 *
 * Read on mount rather than at setup so the first paint is the default and a
 * stored preference lands on top of it, and written on every change rather
 * than on unload so a window closed mid-drag keeps where it was left.
 */
const railWidth = ref(RAIL_PREFERRED_WIDTH)
const inspectorWidth = ref(INSPECTOR_PREFERRED_WIDTH)
// Keep stored widths as preferences, but reserve room for the conversation
// when both panels are visible in a smaller desktop window.
const displayedInspectorWidth = computed(() => !inspectorDocked.value ? inspectorWidth.value : Math.min(
  inspectorWidth.value,
  Math.max(INSPECTOR_MIN_WIDTH, contentWidth.value -
    (railCollapsed.value || narrowViewport.value ? 0 : RAIL_MIN_WIDTH) - TRANSCRIPT_MIN_WIDTH),
))
const displayedRailWidth = computed(() => clampRailWidth(
  railWidth.value,
  contentWidth.value -
    (inspectorDocked.value && props.inspectorOpen ? displayedInspectorWidth.value : 0),
))

onMounted(() => {
  const stored = readShellWidths()
  const viewport = typeof window === 'undefined' ? RAIL_MAX_WIDTH + 360 : window.innerWidth
  if (stored.rail !== null) railWidth.value = clampRailWidth(stored.rail, viewport)
  if (stored.inspector !== null) inspectorWidth.value = clampInspectorWidth(stored.inspector)
})

function viewportWidth(): number {
  return typeof window === 'undefined' ? RAIL_MAX_WIDTH + 360 : contentWidth.value
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

// Deltas preserve the grabbed position, including the separator's hit area and
// any navigation column outside this shell. Use rendered widths after clamping.
let dragging: { pane: 'rail' | 'inspector'; x: number; width: number } | null = null
const resizing = ref(false)

function startDrag(which: 'rail' | 'inspector', event: PointerEvent): void {
  dragging = { pane: which, x: event.clientX, width: which === 'rail' ? displayedRailWidth.value : displayedInspectorWidth.value }
  resizing.value = true
  const target = event.currentTarget as HTMLElement | null
  target?.setPointerCapture?.(event.pointerId)
}

function moveDrag(event: PointerEvent): void {
  if (!dragging) return
  const delta = event.clientX - dragging.x
  if (dragging.pane === 'rail') setRailWidth(dragging.width + delta)
  else setInspectorWidth(dragging.width - delta)
}

function endDrag(): void {
  dragging = null
  resizing.value = false
}

onBeforeUnmount(endDrag)
</script>

<template>
  <div
    ref="shellElement"
    class="app-shell"
    :class="{
      'app-shell--inspector-closed': !props.inspectorOpen,
      'app-shell--rail-closed': railCollapsed,
      'app-shell--resizing': resizing,
    }"
    :data-rail-folded="String(railCollapsed)"
    :data-navigation-in-rail="String(props.navigationInRail)"
    :style="{ '--rail-width': displayedRailWidth + 'px', '--inspector-width': displayedInspectorWidth + 'px' }"
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
        @click="openSessions"
      >
        {{ props.sessionsTitle }}
      </AppButton>
      <AppButton
        variant="ghost"
        size="sm"
        data-mobile-inspector
        :aria-label="props.inspectorTitle"
        @click="updateInspectorDrawer(true)"
      >
        {{ props.inspectorTitle }}
      </AppButton>
    </nav>

    <div class="app-shell__body">
      <div v-if="slots.navigation" class="app-shell__navigation">
        <slot name="navigation" :toggle-sessions="toggleSessions" :sessions-expanded="!railCollapsed" />
      </div>
      <div class="app-shell__grid">
      <nav
        :inert="railCollapsed || narrowViewport"
        :aria-hidden="railCollapsed || narrowViewport"
        class="app-shell__rail"
        data-pane="sessions"
        data-rail
        :data-rail-appearance="props.railAppearance"
        :data-rail-width="displayedRailWidth"
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
          :aria-valuenow="displayedRailWidth"
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
        :aria-label="t('layout.transcript')"
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
        :data-inspector-width="displayedInspectorWidth"
        :hidden="!props.inspectorOpen"
        :aria-label="t('layout.inspector')"
      >
        <span
          class="app-shell__resize"
          data-inspector-resize
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-label="props.inspectorResizeLabel ?? t('layout.resizeInspector')"
          :aria-valuenow="displayedInspectorWidth"
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
    </div>

    <AppDrawer v-model:open="sessionsOpen" :title="props.sessionsTitle" side="left">
      <slot name="sessions" />
    </AppDrawer>
    <AppDrawer :open="inspectorDrawerOpen" :title="props.inspectorTitle" side="right" @update:open="updateInspectorDrawer">
      <slot name="inspector" />
    </AppDrawer>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  min-block-size: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.app-shell__mobile-controls {
  display: none;
}

.app-shell__body {
  display: flex;
  min-inline-size: 0;
  min-block-size: 0;
  flex: 1;
  overflow: hidden;
}

.app-shell__navigation {
  inline-size: 52px;
  flex: 0 0 52px;
}

/* The reference draws one sidebar, not a strip of icons beside a list. A view
   that has moved its destinations into the list itself sets this attribute and
   the strip stops taking a column, so the shell's own vocabulary for "where the
   navigation lives" stays in one place. */
.app-shell[data-navigation-in-rail='true'] .app-shell__navigation {
  inline-size: 0;
  flex-basis: 0;
  overflow: hidden;
}

.app-shell__grid {
  display: grid;
  transition: grid-template-columns 180ms var(--ease-out);
  min-inline-size: 0;
  grid-template-columns:
    var(--rail-width, var(--rail-preferred-width, var(--sidebar-width)))
    minmax(0, 1fr)
    var(--inspector-width, 340px);
  min-block-size: 0;
  flex: 1;
  overflow: hidden;
  margin-inline-end: var(--space-2);
  margin-block-end: var(--space-2);
  border-radius: 16px;
  background: var(--transcript-surface, var(--color-bg-surface));
  box-shadow: 0 1px 5px rgb(0 0 0 / 3%);
}

.app-shell--inspector-closed .app-shell__grid {
  grid-template-columns:
    var(--rail-width, var(--rail-preferred-width, var(--sidebar-width)))
    minmax(0, 1fr);
}

.app-shell--rail-closed .app-shell__grid {
  grid-template-columns: 0px minmax(0, 1fr) var(--inspector-width, 340px);
}

.app-shell--rail-closed.app-shell--inspector-closed .app-shell__grid {
  grid-template-columns: 0px minmax(0, 1fr);
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
  overflow-x: hidden;
  background: var(--color-bg-surface);
}

.app-shell__rail {
  border-inline-end: 1px solid var(--workspace-seam, var(--color-border-subtle));
  overflow: hidden;
}

.app-shell__inspector {
  border-inline-start: 1px solid var(--workspace-seam, var(--color-border-subtle));
}

.app-shell__transcript {
  display: flex;
  flex-direction: column;
  background: var(--transcript-surface, var(--color-bg-base));
}

.app-shell__transcript > :deep(.thread-bar) {
  flex-shrink: 0;
}

/* The handle is the seam itself: a hairline the pointer can still find. */
.app-shell__resize {
  position: absolute;
  inset-block: 0;
  inline-size: 6px;
  cursor: col-resize;
  touch-action: none;
  z-index: 1;
}

.app-shell__resize:hover {
  background: var(--color-accent-border);
}

.app-shell__rail .app-shell__resize {
  inset-inline-end: 0;
}

.app-shell__inspector .app-shell__resize {
  inset-inline-start: 0;
}

.app-shell__resize:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}


.app-shell__rail { overflow: hidden; }
.app-shell--rail-closed .app-shell__rail {
  visibility: hidden;
  pointer-events: none;
  transition: visibility 0s 180ms;
}
.app-shell--resizing .app-shell__grid { transition: none; }
.app-shell__transcript { grid-column: 2; }
.app-shell__inspector { grid-column: 3; }
@media (prefers-reduced-motion: reduce) {
  .app-shell__grid, .app-shell--rail-closed .app-shell__rail { transition: none; }
}

@media (max-width: 1119px) {
  .app-shell .app-shell__grid {
    grid-template-columns: var(--rail-width, 288px) minmax(0, 1fr);
  }

  .app-shell--rail-closed .app-shell__grid {
    grid-template-columns: 0px minmax(0, 1fr);
  }

  .app-shell__inspector {
    display: none;
  }
}

@media (max-width: 799px) {
  .app-shell__navigation {
    display: none;
  }

  .app-shell__mobile-controls {
    display: flex;
    min-block-size: var(--control-height-lg);
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: transparent;
  }

  .app-shell .app-shell__grid {
    grid-template-columns: 0px minmax(0, 1fr);
    min-block-size: 0;
    margin-inline: calc(var(--space-3) / 2);
    margin-block-end: calc(var(--space-3) / 2);
    border-radius: 12px;
  }

  .app-shell__rail,
  .app-shell__inspector {
    display: none;
  }
}
</style>
