import { readStored, writeStored } from '@orchester/design/storage'
import { computed, readonly, ref, type ComputedRef, type Ref } from 'vue'

/**
 * The runs the reader keeps at the top of the rail.
 *
 * The reference's sidebar opens with a pinned list, and this is the reader's own
 * ordering of their work rather than a fact the runtime knows: nothing about a
 * run changes when it is pinned, so it is a local reading preference like the
 * shell's widths and the companion's visibility, and it lives in the same
 * storage they use.
 *
 * Stored as JSON rather than as a delimited string, because an identifier is
 * opaque: whatever the runtime hands out has to survive the round trip, and a
 * separator would eventually be part of one.
 */

export const PINNED_SESSIONS_STORAGE_KEY = 'orchester:sessions:pinned'

/** Reads the stored list, keeping only what could be an identifier. */
export function parsePinnedSessions(value: string | null): string[] {
  if (value === null) return []
  let parsed: unknown
  try {
    parsed = JSON.parse(value)
  } catch {
    return []
  }
  if (!Array.isArray(parsed)) return []
  const ids: string[] = []
  for (const entry of parsed) {
    if (typeof entry !== 'string') continue
    const id = entry.trim()
    if (id.length === 0 || ids.includes(id)) continue
    ids.push(id)
  }
  return ids
}

const pinned = ref<string[]>([])
let initialized = false

/**
 * Read what a previous session pinned, once.
 *
 * Lazily rather than at module load, so the store reads the same storage every
 * other preference reads and a test can seed it before the first use.
 */
function init(): void {
  if (initialized) return
  pinned.value = parsePinnedSessions(readStored(PINNED_SESSIONS_STORAGE_KEY))
  initialized = true
}

function persist(): void {
  writeStored(PINNED_SESSIONS_STORAGE_KEY, JSON.stringify(pinned.value))
}

export interface PinnedSessionsApi {
  /** The pinned identifiers, most recently pinned first. */
  pinned: Readonly<Ref<readonly string[]>>
  pinnedSet: ComputedRef<ReadonlySet<string>>
  isPinned: (id: string) => boolean
  /** Pin an unpinned run, or unpin a pinned one. */
  toggle: (id: string) => void
}

export function usePinnedSessions(): PinnedSessionsApi {
  init()
  const pinnedSet = computed(() => new Set(pinned.value))
  return {
    pinned: readonly(pinned),
    pinnedSet,
    isPinned: (id: string) => pinnedSet.value.has(id),
    toggle: (id: string) => {
      // The newest pin goes to the top: the rail is read from the top, and the
      // run the reader just decided to keep is the one they are looking at.
      pinned.value = pinnedSet.value.has(id)
        ? pinned.value.filter((entry) => entry !== id)
        : [id, ...pinned.value]
      persist()
    },
  }
}

/** Reset the module singleton. Exists for tests, which need a clean store. */
export function resetPinnedSessionsForTests(): void {
  pinned.value = []
  initialized = false
}