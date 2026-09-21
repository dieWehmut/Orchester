import { readStored, writeStored } from '@orchester/design/storage'

/**
 * Where terminal tabs open, task U5-05 of the implementation plan.
 *
 * Section 4.7 gives the inspector a Terminal tab and says a preference chooses
 * whether terminal tabs open in the inspector or the bottom panel; section 4.8
 * gives the bottom panel the same three surfaces. The choice is the reader's,
 * so it lives in the same storage the shell's widths and its panel state use
 * rather than in a view that is thrown away on navigation.
 *
 * The bottom panel is the default because it is the terminal's own surface in
 * the reference layout. A stored value that is not one of the two placements is
 * treated as no value at all, so a hand-edited key opens the default rather
 * than a terminal with nowhere to be.
 */

export const TERMINAL_PLACEMENT_STORAGE_KEY = 'orchester:shell:terminal-placement'

export const TERMINAL_PLACEMENTS = ['bottom', 'inspector'] as const
export type TerminalPlacement = (typeof TERMINAL_PLACEMENTS)[number]

export const DEFAULT_TERMINAL_PLACEMENT: TerminalPlacement = 'bottom'

export function isTerminalPlacement(value: unknown): value is TerminalPlacement {
  return typeof value === 'string' && (TERMINAL_PLACEMENTS as readonly string[]).includes(value)
}

export function readTerminalPlacement(): TerminalPlacement {
  const stored = readStored(TERMINAL_PLACEMENT_STORAGE_KEY)
  return isTerminalPlacement(stored) ? stored : DEFAULT_TERMINAL_PLACEMENT
}

export function writeTerminalPlacement(placement: TerminalPlacement): void {
  writeStored(TERMINAL_PLACEMENT_STORAGE_KEY, placement)
}
