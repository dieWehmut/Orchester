/**
 * The rail's collapsed state, section 2.1 of the design spec.
 *
 * "The rail's collapsed state is a single boolean the user can toggle from
 * anywhere" - so it is one boolean for the whole app rather than a flag each
 * surface keeps. It lives here rather than in the shell because two surfaces
 * read it: the title row draws the toggle's pressed state, and the shell is
 * the thing that actually stops drawing the column.
 *
 * The value is persisted beside the shell's widths and hydrated on first use,
 * for the same reason they are: it is a reading preference, and a preference
 * that resets on reload is a setting the reader has to keep re-taking. The key
 * is a string a reader can edit by hand, so only a value this module can
 * believe is honoured - anything else leaves the rail open.
 */

import { ref } from 'vue'

export const RAIL_COLLAPSED_STORAGE_KEY = 'orchester:rail:collapsed'

function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage
  } catch {
    // A browser that refuses storage must not take the shell down with it.
    return null
  }
}

/** Only the two values this module writes are read back as state. */
export function parseRailCollapsed(raw: string | null): boolean {
  return raw === 'true'
}

export function readRailCollapsed(): boolean {
  const store = storage()
  if (!store) return false
  try {
    return parseRailCollapsed(store.getItem(RAIL_COLLAPSED_STORAGE_KEY))
  } catch {
    return false
  }
}

export function writeRailCollapsed(collapsed: boolean): void {
  const store = storage()
  if (!store) return
  try {
    store.setItem(RAIL_COLLAPSED_STORAGE_KEY, String(collapsed))
  } catch {
    // A full or read-only store is not a reason to fail a toggle.
  }
}

const collapsed = ref(false)
let hydrated = false

export interface RailCollapsedState {
  readonly collapsed: typeof collapsed
  set: (value: boolean) => void
  toggle: () => void
}

/**
 * The one rail boolean.
 *
 * Hydration happens on first read rather than at import, so the module can be
 * imported by a test or a server render without touching storage, and the
 * stored preference lands on top of the open-rail default the way the shell's
 * widths do.
 */
export function useRailCollapsed(): RailCollapsedState {
  if (!hydrated) {
    hydrated = true
    collapsed.value = readRailCollapsed()
  }

  return {
    collapsed,
    set(value: boolean) {
      collapsed.value = value
      writeRailCollapsed(value)
    },
    toggle() {
      const next = !collapsed.value
      collapsed.value = next
      writeRailCollapsed(next)
    },
  }
}

/** Clears the module's memory so a test starts where a first reader does. */
export function resetRailCollapsedForTests(): void {
  collapsed.value = false
  hydrated = false
}
