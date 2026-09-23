<script setup lang="ts">
/**
 * The title row: region A of the shell, and the reference's top strip.
 *
 * The reference draws one row above everything - the rail's toggle, back and
 * forward, the four menus, and the window's caption buttons at the trailing
 * edge. Orchester drew two: a bare title bar and, under it, a product header.
 * The header's content was already stated elsewhere (the rail's product row
 * names the product and the workspace since U26), so the two rows became this
 * one, and the parts of the header that were not duplicated - the connection
 * state and the theme switch - moved here and into the View menu.
 *
 * The row is drawn in every face. What differs between them is what the
 * runtime can answer: the drag region and the caption buttons exist only where
 * there is a native window to move or close, and a chord is printed on a menu
 * row only while the surface behind it has bound that chord.
 */
import { computed, inject, onMounted, onUnmounted, ref } from 'vue'
import { routerKey } from 'vue-router'
import { ArrowLeft, ArrowRight, Maximize2, Minus, PanelLeft, Square, X } from '@lucide/vue'

import { AppMenu, StatusDot, useAppearance, type AppMenuItem } from '@orchester/design'

import { useI18n } from '../../i18n'
import { useRailCollapsed } from '../../composables/use-rail-collapsed'
import { formatShortcut, shortcutRegistry } from '../../shortcuts'
import { desktopWindow, type DesktopWindowController } from '../../platform/desktop-window'
import { useWindowChrome } from '../../composables/use-window-chrome'
import { readNavAvailability, type NavAvailability } from './nav-history'
import {
  hasShellAction,
  runShellAction,
  shellActionExpanded,
  shellActionsVersion,
} from './shell-actions'

export type RuntimeConnection = 'pending' | 'ready' | 'offline' | 'error'

const props = withDefaults(
  defineProps<{
    controller?: DesktopWindowController
    connection?: RuntimeConnection
  }>(),
  {
    connection: 'pending',
  },
)

const { t } = useI18n()
const appRouter = inject(routerKey, null)

const controller = props.controller ?? desktopWindow
const platform = controller.platform ?? 'linux'
const { close, maximized, minimize, toggleMaximize } = useWindowChrome(controller)

const { collapsed: railCollapsed } = useRailCollapsed()
const actionsVersion = shellActionsVersion()
const appearance = useAppearance()
/**
 * Whether the rail is showing.
 *
 * Asked of the surface that answered the toggle rather than read off the
 * stored boolean: the shell draws a column at one viewport width and a drawer
 * at another, and the boolean only knows about the column.
 */
const railExpanded = computed(() => {
  void actionsVersion.value
  return shellActionExpanded('rail.toggle') ?? !railCollapsed.value
})

/**
 * Which of the app chrome's actions exist on the route underneath.
 *
 * The row sits above the routed view, so a pane it could act on belongs to
 * something it cannot see. The mounted surface registers what it can answer -
 * see `shell-actions` - and a row whose action is absent is drawn disabled
 * with its reason rather than offered and then doing nothing.
 */
function available(id: Parameters<typeof hasShellAction>[0]): boolean {
  // The version is what makes this reactive: a register or an unregister is a
  // change the row has to re-render for, and the map itself is not reactive.
  void actionsVersion.value
  return hasShellAction(id)
}

const canFoldRail = computed(() => available('rail.toggle'))

const canToggleInspector = computed(() => available('inspector.toggle'))
const canFocusPrompt = computed(() => available('prompt.focus'))
const canClearPrompt = computed(() => available('prompt.clear'))
const canToggleCompanion = computed(() => available('companion.toggle'))
const canCloseTab = computed(() => available('close-tab'))

/**
 * Whether back and forward are real moves.
 *
 * Read off the history the router is on rather than remembered here, and
 * re-read after every navigation: whether the control is a move or a no-op is
 * a fact about the stack, not about what this row saw last.
 */
const nav = ref<NavAvailability>({ back: false, forward: false })
let stopNavigation: (() => void) | null = null

function readNav(): void {
  nav.value = readNavAvailability(appRouter?.options.history.state)
}

onMounted(() => {
  readNav()
  stopNavigation = appRouter?.afterEach(() => readNav()) ?? null
})

onUnmounted(() => {
  stopNavigation?.()
  stopNavigation = null
})

function goBack(): void {
  if (!nav.value.back) return
  appRouter?.back()
}

function goForward(): void {
  if (!nav.value.forward) return
  appRouter?.forward()
}

function foldRail(): void {
  // Only the surface that drew the rail folds it; this row is above the route
  // and cannot see what the toggle reaches.
  runShellAction('rail.toggle')
}

function openSettings(section: string): void {
  void appRouter?.push({ name: 'settings', query: { section } })
}

/**
 * A new chat is the workspace route with nothing selected.
 *
 * On another route - settings, say - the row has to do both halves: leaving
 * the reader on the screen they were on would be a new chat they cannot see.
 */
function newChat(): void {
  runShellAction('prompt.clear')
  void appRouter?.push({ name: 'workspace' })
}

/**
 * The chord to print on a row, when the product has bound one.
 *
 * Read from the live registry rather than written out: the settings editor
 * lets the reader rebind every one of these, and a menu that kept the default
 * would teach a key that no longer does anything. A chord that is not bound
 * here - `Mod+W` in a browser, where the chord belongs to the tab - prints
 * nothing rather than a key that would not work.
 */
/**
 * The registry's bindings, as a render dependency.
 *
 * The registry is a module singleton with subscribers of its own rather than a
 * reactive store, so a rebinding is an event the row has to hear: without this
 * a menu would keep teaching the shipped chord after the reader changed it.
 */
const shortcutVersion = ref(0)
let stopShortcutWatch: (() => void) | null = null

onMounted(() => {
  stopShortcutWatch = shortcutRegistry.subscribe(() => {
    shortcutVersion.value += 1
  })
})

onUnmounted(() => {
  stopShortcutWatch?.()
  stopShortcutWatch = null
})

function chord(id: string): string | undefined {
  void shortcutVersion.value
  const keys = shortcutRegistry.effectiveKeys(id)
  if (!keys || keys.length === 0) return undefined
  return formatShortcut(keys, platform)
}

const connectionLabel = computed(() => {
  if (props.connection === 'ready') return t('app.connected')
  if (props.connection === 'offline') return t('app.offline')
  if (props.connection === 'error') return t('app.connectionError')
  return t('app.runtimePending')
})
const connectionStatus = computed(() => {
  if (props.connection === 'ready') return 'success' as const
  if (props.connection === 'error') return 'error' as const
  if (props.connection === 'pending') return 'running' as const
  return 'idle' as const
})

const railLabel = computed(() =>
  railExpanded.value ? t('titleRow.hideRail') : t('titleRow.showRail'),
)

const unavailable = computed(() => t('titleRow.unavailable'))

/**
 * One menu row.
 *
 * A hint or a reason is either present or absent, and absent has to be absent
 * rather than `undefined`: the item type is exact, so a row that passed the
 * key with no value would be a row carrying a chord-shaped hole.
 */
function menuItem(id: string, label: string, hint?: string, disabled?: boolean): AppMenuItem {
  return {
    id,
    label,
    ...(hint === undefined ? {} : { hint }),
    ...(disabled === true ? { disabled: true } : {}),
  }
}

/**
 * The File menu.
 *
 * A new chat is the workspace with nothing selected, settings is the route the
 * rail's own gear opens, and closing the active tab is the chord the strip
 * answers - offered as a row only while there is a tab to close, because the
 * transcript cannot be closed and the window is not a tab.
 */
const fileItems = computed<AppMenuItem[]>(() => [
  menuItem('new-chat', t('sessions.newChat'), chord('session.new')),
  menuItem('settings', t('settings.title'), chord('settings.open')),
  menuItem(
    'close-tab',
    t('shortcuts.labels.windowClose'),
    canCloseTab.value ? chord('window.close') : unavailable.value,
    !canCloseTab.value,
  ),
])

/**
 * The Edit menu.
 *
 * Orchester owns no editor, so the platform's own clipboard and selection rows
 * are deliberately absent: they would be a menu claiming work the webview
 * already does. What the product can honestly answer is where the reader types.
 */
const editItems = computed<AppMenuItem[]>(() => [
  menuItem(
    'focus-prompt',
    t('shortcuts.labels.composerFocus'),
    canFocusPrompt.value ? chord('composer.focus') : unavailable.value,
    !canFocusPrompt.value,
  ),
  menuItem(
    'clear-prompt',
    t('titleRow.clearPrompt'),
    canClearPrompt.value ? undefined : unavailable.value,
    !canClearPrompt.value,
  ),
])

/**
 * The View menu.
 *
 * The three panes the reference toggles, and the theme: the appearance screen
 * owns the theme, so the row offers the one step a menu can take and says which
 * way it goes. A pane this route does not draw is disabled with the reason
 * rather than offered and then doing nothing.
 */
const viewItems = computed<AppMenuItem[]>(() => [
  menuItem(
    'toggle-rail',
    railLabel.value,
    canFoldRail.value ? chord('rail.toggle') : unavailable.value,
    !canFoldRail.value,
  ),
  menuItem(
    'toggle-inspector',
    t('shortcuts.labels.inspectorToggle'),
    canToggleInspector.value ? chord('inspector.toggle') : unavailable.value,
    !canToggleInspector.value,
  ),
  menuItem(
    'toggle-companion',
    t('shortcuts.labels.companionToggle'),
    canToggleCompanion.value ? chord('companion.toggle') : unavailable.value,
    !canToggleCompanion.value,
  ),
  menuItem(
    'toggle-theme',
    t(appearance.isDark.value ? 'settings.theme.toLight' : 'settings.theme.toDark'),
  ),
])

const helpItems = computed<AppMenuItem[]>(() => [
  menuItem('keybindings', t('settings.sections.keybindings')),
  menuItem('about', t('settings.sections.about')),
])

function onFileSelect(id: string): void {
  if (id === 'new-chat') newChat()
  else if (id === 'settings') openSettings('appearance')
  else if (id === 'close-tab') runShellAction('close-tab')
}

function onEditSelect(id: string): void {
  if (id === 'focus-prompt') runShellAction('prompt.focus')
  else if (id === 'clear-prompt') runShellAction('prompt.clear')
}

function onViewSelect(id: string): void {
  if (id === 'toggle-rail') foldRail()
  else if (id === 'toggle-inspector') runShellAction('inspector.toggle')
  else if (id === 'toggle-companion') runShellAction('companion.toggle')
  else if (id === 'toggle-theme') appearance.toggleTheme()
}

function onHelpSelect(id: string): void {
  if (id === 'keybindings') openSettings('keybindings')
  else if (id === 'about') openSettings('about')
}
</script>

<template>
  <header
    class="title-row"
    :class="{ 'title-row--desktop': controller.enabled }"
    data-title-row
    :data-window-chrome="controller.enabled ? '' : undefined"
    :data-window-material="controller.enabled ? 'opaque' : undefined"
    :data-window-platform="controller.enabled ? platform : undefined"
  >
    <span v-if="controller.enabled && platform === 'macos'" class="title-row__traffic-lights" data-native-traffic-lights aria-hidden="true" />

    <div
      class="title-row__controls"
      :data-tauri-drag-region="controller.enabled ? 'deep' : undefined"
      :title="t('app.name')"
    >
      <button
        class="title-row__control"
        data-rail-toggle
        type="button"
        :aria-label="railLabel"
        :title="railLabel"
        :aria-expanded="railExpanded"
        :disabled="!canFoldRail"
        @click="foldRail"
      >
        <PanelLeft :size="16" :stroke-width="1.8" aria-hidden="true" />
      </button>

      <button
        class="title-row__control"
        data-title-nav="back"
        type="button"
        :aria-label="t('titleRow.back')"
        :title="t('titleRow.back')"
        :disabled="!nav.back"
        @click="goBack"
      >
        <ArrowLeft :size="16" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button
        class="title-row__control"
        data-title-nav="forward"
        type="button"
        :aria-label="t('titleRow.forward')"
        :title="t('titleRow.forward')"
        :disabled="!nav.forward"
        @click="goForward"
      >
        <ArrowRight :size="16" :stroke-width="1.8" aria-hidden="true" />
      </button>

      <nav class="title-row__menus" data-title-menus :aria-label="t('titleRow.menusLabel')">
        <AppMenu
          class="title-row__menu"
          :label="t('titleRow.file')"
          data-title-menu="file"
          :items="fileItems"
          @select="onFileSelect"
        />
        <AppMenu
          class="title-row__menu"
          :label="t('titleRow.edit')"
          data-title-menu="edit"
          :items="editItems"
          @select="onEditSelect"
        />
        <AppMenu
          class="title-row__menu"
          :label="t('titleRow.view')"
          data-title-menu="view"
          :items="viewItems"
          @select="onViewSelect"
        />
        <AppMenu
          class="title-row__menu"
          :label="t('titleRow.help')"
          data-title-menu="help"
          :items="helpItems"
          @select="onHelpSelect"
        />
      </nav>

      <div v-if="connection !== 'ready'" class="title-row__connection">
        <StatusDot
          :status="connectionStatus"
          :label="connectionLabel"
          :pulse="connection === 'pending'"
        />
        <span data-testid="connection-label">{{ connectionLabel }}</span>
      </div>
    </div>

    <div
      v-if="controller.enabled && platform !== 'macos'"
      class="title-row__captions"
      :aria-label="t('window.controls')"
    >
      <button
        class="title-row__caption"
        data-window-action="minimize"
        type="button"
        :aria-label="t('window.minimize')"
        :title="t('window.minimize')"
        @click="minimize"
      >
        <Minus :size="15" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button
        class="title-row__caption"
        data-window-action="maximize"
        :data-native-snap-target="platform === 'windows' ? 'true' : undefined"
        type="button"
        :aria-label="maximized ? t('window.restore') : t('window.maximize')"
        :title="maximized ? t('window.restore') : t('window.maximize')"
        @click="toggleMaximize"
      >
        <Square v-if="!maximized" :size="13" :stroke-width="1.8" aria-hidden="true" />
        <Maximize2 v-else :size="14" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button
        class="title-row__caption title-row__caption--close"
        data-window-action="close"
        type="button"
        :aria-label="t('window.close')"
        :title="t('window.close')"
        @click="close"
      >
        <X :size="15" :stroke-width="1.8" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>

<style scoped>
/* The row is region A: the app chrome in every face, and the window's own
   title bar where there is a window. Its height is the chrome height the
   full-height surfaces subtract, so the row and the layouts under it cannot
   disagree about how much room the top takes. */
.title-row {
  position: sticky;
  inset-block-start: 0;
  z-index: var(--z-header);
  display: flex;
  block-size: var(--app-chrome-height);
  min-block-size: var(--app-chrome-height);
  align-items: stretch;
  justify-content: space-between;
  border-block-end: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
  color: var(--color-text-secondary);
  user-select: none;
}

/* CSS blur support does not prove native compositor availability. The shell
   supplies opaque windows on every supported OS, so the row states the
   material it is rather than the one it might have had. */
.title-row[data-window-material='opaque'] {
  background: var(--color-surface-base);
  backdrop-filter: none;
}

.title-row__traffic-lights {
  flex: 0 0 80px;
}

.title-row__controls {
  display: flex;
  min-inline-size: 0;
  flex: 1 1 auto;
  align-items: center;
  gap: var(--space-1);
  padding-inline: var(--space-2);
}

/* The row's own controls sit on it rather than in boxes: the reference's
   strip is glyphs on the chrome, and a bordered button on a title bar reads
   as a toolbar rather than as chrome. */
.title-row__control {
  display: inline-flex;
  min-inline-size: var(--hit-target-min, 32px);
  min-block-size: var(--hit-target-min, 32px);
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
}

.title-row__control:hover:not(:disabled) {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.title-row__control[aria-pressed='true'] {
  background: var(--color-accent-muted);
  color: var(--color-accent);
}

.title-row__control:disabled {
  color: var(--color-text-disabled);
  cursor: not-allowed;
}

.title-row__control:focus-visible,
.title-row__caption:focus-visible,
.title-row__menu:focus-within {
  outline: 2px solid var(--color-border-focus);
  outline-offset: -2px;
}

.title-row__menus {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: 2px;
}

/* A menu bar trigger is a label, not a control with a frame: the reference
   sets the four names flat on the row, and the surface that opens is what
   says they are menus. */
.title-row__menu :deep(.app-menu__trigger) {
  padding: 0 var(--space-2);
  border-color: transparent;
  background: transparent;
  font-size: var(--text-sm);
}

.title-row__menu :deep(.app-menu__trigger:hover) {
  background: var(--color-bg-element);
}

.title-row__connection {
  display: flex;
  min-block-size: var(--control-height-sm);
  align-items: center;
  gap: var(--space-2);
  margin-inline-start: auto;
  padding-inline: var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  white-space: nowrap;
}

.title-row__captions {
  display: flex;
  flex: 0 0 auto;
  align-items: start;
}

/* Windows measures these, so they are the OS metric rather than the shared
   floor: 46 by 32 is what the native hit-testing in the shell's own crate
   assumes when it hands Snap Layouts the maximize button. */
.title-row__caption {
  display: grid;
  inline-size: 46px;
  min-inline-size: 46px;
  block-size: 32px;
  min-block-size: 32px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: default;
}

.title-row__caption:hover {
  background: var(--color-bg-element);
}

.title-row__caption--close:hover {
  background: var(--color-intent-danger-solid);
  color: var(--color-text-inverse);
}

@media (max-width: 640px) {
  .title-row__connection span {
    display: none;
  }

  .title-row__connection {
    padding-inline: var(--space-2);
  }
}
</style>
