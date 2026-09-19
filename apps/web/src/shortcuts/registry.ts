/**
 * The keyboard-shortcut registry.
 *
 * A shortcut editor is only honest if it reads the bindings the components
 * actually use. Keeping a second hand-written list in the editor would drift
 * the first time a component changed its keys, so every component registers
 * here and the editor renders what the registry holds. The registry also owns
 * the default bindings, so "reset all" has something to reset to.
 */

import type { Platform } from '@orchester/design'

export type ShortcutKey = string

export interface ShortcutDefinition {
  /** Stable identity, used for rebinding and reset. */
  readonly id: string
  /** What the shortcut does, in the words the editor shows. */
  readonly label: string
  /** The part of the app it belongs to, for grouping in the editor. */
  readonly group: string
  /** `Mod` is Cmd on macOS and Ctrl everywhere else. */
  readonly keys: readonly ShortcutKey[]
}

export type ShortcutRegistration = ShortcutDefinition

export interface ShortcutEvent {
  readonly key: string
  readonly metaKey: boolean
  readonly ctrlKey: boolean
  readonly shiftKey: boolean
  readonly altKey: boolean
}

/**
 * The bindings Orchester ships with.
 *
 * Every one of these has to be answered by a registered handler: the registry
 * test asserts there are no duplicate bindings, and the editor cannot show a
 * shortcut that no component claims.
 */
export const DEFAULT_SHORTCUTS: readonly ShortcutDefinition[] = [
  {
    id: 'palette.open',
    label: 'Open the command palette',
    group: 'Composer',
    keys: ['Mod', 'K'],
  },
  {
    id: 'composer.focus',
    label: 'Focus the composer',
    group: 'Composer',
    keys: ['Mod', 'L'],
  },
  {
    id: 'session.new',
    label: 'Start a new session',
    group: 'Sessions',
    keys: ['Mod', 'N'],
  },
  {
    id: 'inspector.toggle',
    label: 'Toggle the inspector',
    group: 'Layout',
    keys: ['Mod', 'B'],
  },
  {
    id: 'settings.open',
    label: 'Open settings',
    group: 'Layout',
    keys: ['Mod', ','],
  },
  {
    id: 'tab.close',
    label: 'Close the active tab',
    group: 'Layout',
    keys: ['Mod', 'W'],
  },
]

const MODIFIER_LABELS: Record<Platform, string> = {
  macos: 'Cmd',
  windows: 'Ctrl',
  linux: 'Ctrl',
}

/**
 * Renders the keys as a reader would press them, on the platform they are on.
 *
 * The stored form is `Mod`, which is not a key on any keyboard; showing it
 * verbatim would make the editor unreadable and the platform check would be
 * hidden in a string nobody prints.
 */
export function formatShortcut(keys: readonly ShortcutKey[], platform: Platform): string {
  return keys.map((key) => (key === 'Mod' ? MODIFIER_LABELS[platform] : key)).join('+')
}

/**
 * Whether a keyboard event is the given shortcut on this platform.
 *
 * Modifiers have to match exactly: on a Mac `Ctrl+K` is a different binding
 * from `Cmd+K`, and treating them as the same would make the two impossible to
 * hold at once, which is exactly what a shortcut editor has to allow.
 */
export function matchShortcut(
  keys: readonly ShortcutKey[],
  event: ShortcutEvent,
  platform: Platform,
): boolean {
  const wantsMod = keys.includes('Mod')
  const wantsShift = keys.includes('Shift')
  const wantsAlt = keys.includes('Alt')
  const wanted = keys[keys.length - 1]

  const modHeld = platform === 'macos' ? event.metaKey : event.ctrlKey
  const foreignModHeld = platform === 'macos' ? event.ctrlKey : event.metaKey

  if (modHeld !== wantsMod) return false
  if (foreignModHeld) return false
  if (event.shiftKey !== wantsShift) return false
  if (event.altKey !== wantsAlt) return false

  return event.key.toLowerCase() === (wanted ?? '').toLowerCase()
}

export interface ShortcutRegistry {
  register: (shortcut: ShortcutRegistration) => () => void
  list: () => readonly ShortcutRegistration[]
  effectiveKeys: (id: string) => readonly ShortcutKey[] | undefined
  rebind: (id: string, keys: readonly ShortcutKey[]) => void
  resetAll: () => void
}

function serialize(keys: readonly ShortcutKey[]): string {
  return keys.join('+')
}

export function createShortcutRegistry(): ShortcutRegistry {
  const registrations = new Map<string, ShortcutRegistration>()
  const overrides = new Map<string, readonly ShortcutKey[]>()

  /**
   * A user's rebinding wins over the component's own keys. Nothing else does:
   * the editor renders what the component registered, so a component that
   * changes its keys cannot leave a stale binding behind in the editor.
   */
  function keysFor(id: string): readonly ShortcutKey[] {
    return overrides.get(id) ?? registrations.get(id)?.keys ?? []
  }

  function findConflict(id: string, keys: readonly ShortcutKey[]): string | undefined {
    for (const otherId of registrations.keys()) {
      if (otherId === id) continue
      if (serialize(keysFor(otherId)) === serialize(keys)) return otherId
    }
    return undefined
  }

  return {
    register(shortcut) {
      const effective = overrides.get(shortcut.id) ?? shortcut.keys
      const conflict = findConflict(shortcut.id, effective)
      if (conflict !== undefined) {
        throw new Error(
          `The shortcut ${serialize(effective)} is already bound to ${conflict}.`,
        )
      }
      registrations.set(shortcut.id, shortcut)
      return () => {
        registrations.delete(shortcut.id)
      }
    },

    list() {
      return [...registrations.values()]
    },

    effectiveKeys(id) {
      if (!registrations.has(id)) return undefined
      return keysFor(id)
    },

    rebind(id, keys) {
      if (!registrations.has(id)) return
      const conflict = findConflict(id, keys)
      if (conflict !== undefined) {
        throw new Error(
          `The shortcut ${serialize(keys)} is already bound to ${conflict}.`,
        )
      }
      overrides.set(id, [...keys])
    },

    resetAll() {
      overrides.clear()
    },
  }
}
