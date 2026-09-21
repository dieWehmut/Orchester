import { readStored, writeStored } from '@orchester/design/storage'

/**
 * The bottom panel's remembered state, section 2 of the design spec.
 *
 * The panel is the shell's third collapsible region, and like the two widths it
 * is a preference rather than a view state: a user who keeps the terminal open
 * expects it open next time, on the surface they left it on. A stored value
 * that is not one of the three surfaces is treated as no value at all, so a
 * hand-edited key opens the default panel rather than an empty one.
 */

export const BOTTOM_PANEL_STATE_STORAGE_KEY = 'orchester:shell:bottom-panel'

export const BOTTOM_PANEL_MIN_HEIGHT = 160
export const BOTTOM_PANEL_DEFAULT_HEIGHT = 240
/** The ceiling is a share of the viewport, as section 2.1 states it. */
export const BOTTOM_PANEL_MAX_VIEWPORT_SHARE = 0.7

export interface BottomPanelState {
  expanded: boolean
  tab: string
}

export function clampBottomPanelHeight(height: number, viewportHeight: number): number {
  const ceiling = Math.round(viewportHeight * BOTTOM_PANEL_MAX_VIEWPORT_SHARE)
  return Math.round(
    Math.min(Math.max(BOTTOM_PANEL_MIN_HEIGHT, ceiling), Math.max(BOTTOM_PANEL_MIN_HEIGHT, height)),
  )
}

export function readBottomPanelState(): BottomPanelState {
  const fallback: BottomPanelState = { expanded: false, tab: 'terminal' }
  const stored = readStored(BOTTOM_PANEL_STATE_STORAGE_KEY)
  if (stored === null) return fallback
  try {
    const parsed: unknown = JSON.parse(stored)
    if (typeof parsed !== 'object' || parsed === null) return fallback
    const candidate = parsed as { expanded?: unknown; tab?: unknown }
    return {
      expanded: candidate.expanded === true,
      tab: typeof candidate.tab === 'string' && candidate.tab.length > 0 ? candidate.tab : 'terminal',
    }
  } catch {
    return fallback
  }
}

export function writeBottomPanelState(state: BottomPanelState): void {
  writeStored(BOTTOM_PANEL_STATE_STORAGE_KEY, JSON.stringify({ expanded: state.expanded, tab: state.tab }))
}
