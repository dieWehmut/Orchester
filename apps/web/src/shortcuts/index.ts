/**
 * The application's keyboard shortcuts, in one place.
 *
 * Two things have to agree for a shortcut to work: the registry says it is
 * bound, and something answers it. Both are set up here so a component can ask
 * for a shortcut without caring which of the two it is talking to, and so the
 * window listener is installed exactly once.
 */

import { readSystemPlatform, type Platform } from '@orchester/design'
import { onMounted, onUnmounted } from 'vue'

import { createShortcutDispatch, type ShortcutHandler } from './dispatch'
import { createShortcutRegistry, type ShortcutDefinition } from './registry'

export interface ShortcutScopeOptions {
  /** Overridden in tests; the app reads the platform from the document. */
  platform?: () => Platform
}

export interface ShortcutScope {
  registry: ReturnType<typeof createShortcutRegistry>
  dispatch: ReturnType<typeof createShortcutDispatch>
  register: (definition: ShortcutDefinition, handler: ShortcutHandler) => void
}

/**
 * Creates the scope without touching the DOM, so a test can drive the same
 * registry and dispatcher the shell uses.
 */
export function createShortcutScope(options: ShortcutScopeOptions = {}): ShortcutScope {
  const platform = options.platform ?? readSystemPlatform
  const registry = createShortcutRegistry()
  const dispatch = createShortcutDispatch({ registry, platform })

  return {
    registry,
    dispatch,
    register(definition, handler) {
      registry.register(definition)
      dispatch.bind(definition.id, handler)
    },
  }
}

const scope = createShortcutScope()

export const shortcutRegistry = scope.registry

/**
 * Registers a shortcut for the lifetime of the calling component.
 *
 * Registering rather than handing the keys to the caller keeps the editor's
 * list honest: a shortcut exists exactly while a mounted component answers it.
 */
export function useShortcut(definition: ShortcutDefinition, handler: ShortcutHandler): void {
  const unregister = scope.registry.register(definition)
  const unbind = scope.dispatch.bind(definition.id, handler)

  onUnmounted(() => {
    unbind()
    unregister()
  })
}

/**
 * Installs the single window listener. Mounted once, by the shell.
 *
 * Listening on the window rather than on a region means a shortcut works from
 * wherever focus happens to be, which is the point of a global shortcut. The
 * listener captures so a component that stops the event cannot silently hide a
 * shortcut that the editor still lists.
 */
export function useShortcutListener(target: Window | null = typeof window === 'undefined' ? null : window): void {
  onMounted(() => {
    target?.addEventListener('keydown', onKeydown)
  })

  onUnmounted(() => {
    target?.removeEventListener('keydown', onKeydown)
  })
}

function onKeydown(event: KeyboardEvent): void {
  scope.dispatch.handle(event)
}

export { DEFAULT_SHORTCUTS, formatShortcut, matchShortcut } from './registry'
export type { ShortcutDefinition, ShortcutKey, ShortcutRegistration } from './registry'
export type { ShortcutHandler } from './dispatch'
