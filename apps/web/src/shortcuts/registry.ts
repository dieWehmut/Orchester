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

import type { MessageKey } from '../i18n'

export type ShortcutKey = string

export interface ShortcutDefinition {
  /** Stable identity, used for rebinding and reset. */
  readonly id: string
  /**
   * What the shortcut does, as a locale key rather than as words.
   *
   * The registry is shared vocabulary: the WebUI renders these in the reader's
   * language, and a second client with no locale service can still read the
   * identity it needs. Words stored here would be a third copy to translate.
   */
  readonly labelKey: MessageKey
  /** The part of the app it belongs to, for grouping in the editor. */
  readonly groupKey: MessageKey
  /** `Mod` is Cmd on macOS and Ctrl everywhere else. */
  readonly keys: readonly ShortcutKey[]
}

export type ShortcutRegistration = ShortcutDefinition

/**
 * The chord a rebinding asked for is already taken.
 *
 * A conflict is data rather than a sentence: the registry knows which binding
 * kept the chord, and the surface that can translate is the one that renders
 * the message. A string thrown from here could only be English.
 */
export class ShortcutConflictError extends Error {
  readonly keys: readonly ShortcutKey[]
  readonly otherId: string
  readonly otherLabelKey: MessageKey

  constructor(keys: readonly ShortcutKey[], otherId: string, otherLabelKey: MessageKey) {
    super(`The shortcut ${keys.join('+')} is already bound to ${otherId}.`)
    this.name = 'ShortcutConflictError'
    this.keys = [...keys]
    this.otherId = otherId
    this.otherLabelKey = otherLabelKey
  }
}

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
 * `Mod+W` belongs to the shell rather than to the strip: it closes whatever
 * tab the reader is looking at, and it is what hides the window once the
 * transcript - the one tab the window exists for - is the one in front.
 *
 * Every one of these has to be answered by a registered handler: the registry
 * test asserts there are no duplicate bindings, and the editor cannot show a
 * shortcut that no component claims.
 */
export const DEFAULT_SHORTCUTS: readonly ShortcutDefinition[] = [
  {
    id: 'palette.open',
    labelKey: 'shortcuts.labels.paletteOpen',
    groupKey: 'shortcuts.groups.composer',
    keys: ['Mod', 'K'],
  },
  {
    id: 'composer.focus',
    labelKey: 'shortcuts.labels.composerFocus',
    groupKey: 'shortcuts.groups.composer',
    keys: ['Mod', 'L'],
  },
  {
    id: 'session.new',
    labelKey: 'shortcuts.labels.sessionNew',
    groupKey: 'shortcuts.groups.sessions',
    keys: ['Mod', 'N'],
  },
  {
    // Section 2.1 gives the rail's collapse chord to `Mod+B`, and the
    // inspector keeps a chord one modifier over: a product may not hand one
    // gesture to two panes, and the reference's own View menu teaches both.
    id: 'rail.toggle',
    labelKey: 'shortcuts.labels.railToggle',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', 'B'],
  },
  {
    id: 'inspector.toggle',
    labelKey: 'shortcuts.labels.inspectorToggle',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', 'Alt', 'B'],
  },
  {
    id: 'settings.open',
    labelKey: 'shortcuts.labels.settingsOpen',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', ','],
  },
  {
    id: 'companion.toggle',
    labelKey: 'shortcuts.labels.companionToggle',
    groupKey: 'shortcuts.groups.layout',
    // The reference spells this Alt+Win+P. Win is not a key the app can claim:
    // Windows keeps it as the OS chord (Win+P is projection mode), and the
    // registry refuses the foreign modifier on purpose. Mod+Alt+P is the same
    // gesture one key over, and it is a chord the shell can actually answer.
    keys: ['Mod', 'Alt', 'P'],
  },
  {
    id: 'window.close',
    labelKey: 'shortcuts.labels.windowClose',
    groupKey: 'shortcuts.groups.layout',
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
  /** The registered shortcut an event is asking for, if any. */
  match: (event: ShortcutEvent, platform: Platform) => ShortcutRegistration | undefined
  /** Notified whenever the set of bindings or their keys change. */
  subscribe: (listener: () => void) => () => void
}

/** Keys that only exist to modify another key. */
const MODIFIER_KEY_NAMES = new Set(['Alt', 'AltGraph', 'Control', 'Meta', 'Shift'])

/**
 * Turns the next keypress into a portable binding.
 *
 * The modifier is stored as `Mod` rather than as whichever key the platform
 * happens to use, so a binding made on a Mac still works when the same profile
 * is opened on Windows. A chord with no modifier is refused: binding `K` would
 * make the letter unusable everywhere the app listens.
 */
export function captureShortcut(
  event: ShortcutEvent,
  platform: Platform,
): readonly ShortcutKey[] | undefined {
  if (MODIFIER_KEY_NAMES.has(event.key)) return undefined

  const modHeld = platform === 'macos' ? event.metaKey : event.ctrlKey
  if (!modHeld) return undefined

  const keys: ShortcutKey[] = ['Mod']
  if (event.shiftKey) keys.push('Shift')
  if (event.altKey) keys.push('Alt')
  keys.push(event.key.length === 1 ? event.key.toUpperCase() : event.key)
  return keys
}

function serialize(keys: readonly ShortcutKey[]): string {
  return keys.join('+')
}

export function createShortcutRegistry(): ShortcutRegistry {
  const registrations = new Map<string, ShortcutRegistration>()
  const overrides = new Map<string, readonly ShortcutKey[]>()
  const listeners = new Set<() => void>()

  function notify(): void {
    for (const listener of listeners) listener()
  }

  /**
   * A user's rebinding wins over the component's own keys. Nothing else does:
   * the editor renders what the component registered, so a component that
   * changes its keys cannot leave a stale binding behind in the editor.
   */
  function keysFor(id: string): readonly ShortcutKey[] {
    return overrides.get(id) ?? registrations.get(id)?.keys ?? []
  }

  function findConflict(id: string, keys: readonly ShortcutKey[]): ShortcutDefinition | undefined {
    for (const other of registrations.values()) {
      if (other.id === id) continue
      if (serialize(keysFor(other.id)) === serialize(keys)) return other
    }
    return undefined
  }

  return {
    register(shortcut) {
      const effective = overrides.get(shortcut.id) ?? shortcut.keys
      const conflict = findConflict(shortcut.id, effective)
      if (conflict !== undefined) {
        throw new ShortcutConflictError(effective, conflict.id, conflict.labelKey)
      }
      registrations.set(shortcut.id, shortcut)
      notify()
      return () => {
        registrations.delete(shortcut.id)
        notify()
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
        throw new ShortcutConflictError(keys, conflict.id, conflict.labelKey)
      }
      overrides.set(id, [...keys])
      notify()
    },

    resetAll() {
      overrides.clear()
      notify()
    },

    match(event, platform) {
      for (const registration of registrations.values()) {
        if (matchShortcut(keysFor(registration.id), event, platform)) return registration
      }
      return undefined
    },

    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}
