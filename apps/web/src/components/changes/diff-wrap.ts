import { readStored, writeStored } from '@orchester/design/storage'

/**
 * The diff line-wrap preference, task U5-03 of the implementation plan.
 *
 * Section 4.7 asks for a line-wrap toggle on diffs. The choice belongs to the
 * reader rather than to one file, so it lives in the same storage the shell's
 * widths and the appearance axes use: a reader who wraps one long diff has said
 * how they read diffs, and re-deciding it per file is the friction the toggle
 * exists to remove.
 *
 * Unwrapped is the default because a diff is read in columns; wrapping is the
 * exception a reader asks for.
 */

export const DIFF_WRAP_STORAGE_KEY = 'orchester:diff:wrap'

/** Whether the reader has asked for wrapped diff lines. */
export function readDiffWrap(): boolean {
  return readStored(DIFF_WRAP_STORAGE_KEY) === 'on'
}

export function writeDiffWrap(wrapped: boolean): void {
  writeStored(DIFF_WRAP_STORAGE_KEY, wrapped ? 'on' : 'off')
}
