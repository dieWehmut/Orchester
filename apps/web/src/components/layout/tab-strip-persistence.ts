import { readStored, writeStored } from '@orchester/design/storage'

/**
 * The tab strip's remembered state, section 4.2 of the design spec.
 *
 * The spec says the runtime must persist open tabs, and a tab is a fact about
 * the shell rather than about the work inside it, so it is remembered beside
 * the widths the shell already stores. The order is kept as the ids the shell
 * mounts rather than as whole tab records: a tab's label is retitled by the run
 * it names, and a stored label would then be a title the runtime no longer
 * agrees with.
 *
 * A stored payload that is not the shape this module wrote is no state at all,
 * so a hand-edited key opens the default strip rather than half a broken one.
 */

export const TAB_STRIP_STATE_STORAGE_KEY = 'orchester:shell:tab-strip'

export interface TabStripState {
  order: readonly string[]
  activeId: string | null
}

const EMPTY: TabStripState = { order: [], activeId: null }

function isId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

export function readTabStripState(): TabStripState {
  const stored = readStored(TAB_STRIP_STATE_STORAGE_KEY)
  if (stored === null) return EMPTY
  try {
    const parsed: unknown = JSON.parse(stored)
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return EMPTY
    const { order, activeId } = parsed as { order?: unknown; activeId?: unknown }
    if (!Array.isArray(order) || !order.every(isId)) return EMPTY
    return { order, activeId: isId(activeId) ? activeId : null }
  } catch {
    return EMPTY
  }
}

export function writeTabStripState(state: TabStripState): void {
  const order = [...state.order].filter(isId)
  writeStored(
    TAB_STRIP_STATE_STORAGE_KEY,
    JSON.stringify({ order, activeId: isId(state.activeId) ? state.activeId : null }),
  )
}
