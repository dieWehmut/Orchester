/**
 * Shortcut dispatch.
 *
 * The registry says what is bound; this says who answers. Handlers live here
 * rather than in the registry so a component can register its keys once and
 * still have the action belong to the component that owns it. Dispatch is the
 * only place that decides whether a key event is a command at all, which is
 * what keeps the typing rule in one place instead of in every handler.
 */

import type { Platform } from '@orchester/design'

import {
  matchShortcut,
  type ShortcutEvent,
  type ShortcutRegistry,
} from './registry'

export type ShortcutHandler = (event: KeyboardEvent) => void

export interface ShortcutDispatchOptions {
  readonly registry: ShortcutRegistry
  /** Resolved at call time: the platform can change under a running app. */
  readonly platform: () => Platform
}

export interface ShortcutDispatch {
  bind: (id: string, handler: ShortcutHandler) => () => void
  handle: (event: KeyboardEvent) => boolean
  unbindAll: () => void
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!target || typeof target !== 'object') return false
  const element = target as { tagName?: unknown; isContentEditable?: unknown }
  if (element.isContentEditable === true) return true
  const tag = typeof element.tagName === 'string' ? element.tagName.toUpperCase() : ''
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT'
}

function carriesModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey || event.altKey
}

export function createShortcutDispatch(options: ShortcutDispatchOptions): ShortcutDispatch {
  const handlers = new Map<string, ShortcutHandler>()

  return {
    bind(id, handler) {
      handlers.set(id, handler)
      return () => {
        if (handlers.get(id) === handler) handlers.delete(id)
      }
    },

    handle(event) {
      const shortcut: ShortcutEvent = {
        key: event.key,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
      }
      const platform = options.platform()

      for (const registration of options.registry.list()) {
        const keys = options.registry.effectiveKeys(registration.id) ?? registration.keys
        if (!matchShortcut(keys, shortcut, platform)) continue

        // A key with no modifier while the reader is typing is a character,
        // not a command. A chord cannot be typed, so it stays a command.
        if (isEditableTarget(event.target) && !carriesModifier(event)) continue

        const handler = handlers.get(registration.id)
        if (!handler) continue

        event.preventDefault()
        handler(event)
        return true
      }

      return false
    },

    unbindAll() {
      handlers.clear()
    },
  }
}
