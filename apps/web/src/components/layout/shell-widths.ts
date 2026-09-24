import { readStored, writeStored } from '@orchester/design/storage'

/**
 * The shell's resizable width, section 2.1 of the design spec.
 *
 * A width is a clamp, not a number: the rail has a floor it may not go under, a
 * ceiling it may not go over, and a ceiling it may not take from the transcript.
 * There is one width to clamp now that the shell has no right column. The clamp is applied here rather than in
 * the stylesheet because the pointer can travel further than the clamp allows
 * and a stylesheet cannot refuse a value it was handed.
 *
 * The widths are a user preference, so they live in the same storage the
 * appearance axes use. A value that is not a width is treated as no value at
 * all: a stored `wide` should open the default rail, not a broken one.
 */

export const RAIL_WIDTH_STORAGE_KEY = 'orchester:shell:rail-width'

export const RAIL_MIN_WIDTH = 240
export const RAIL_MAX_WIDTH = 420
export const RAIL_PREFERRED_WIDTH = 288
export const TRANSCRIPT_MIN_WIDTH = 360

/** How far one arrow key moves a handle. */
export const RESIZE_STEP = 16

/** The rail's own clamp, and the transcript's claim on the same space. */
export function clampRailWidth(width: number, viewportWidth: number): number {
  const byTranscript = Math.max(RAIL_MIN_WIDTH, viewportWidth - TRANSCRIPT_MIN_WIDTH)
  return Math.round(Math.min(RAIL_MAX_WIDTH, Math.max(RAIL_MIN_WIDTH, Math.min(width, byTranscript))))
}

function readWidth(key: string): number | null {
  const stored = readStored(key)
  if (stored === null) return null
  const width = Number(stored)
  return Number.isFinite(width) && width > 0 ? width : null
}

export interface ShellWidths {
  rail: number | null
}

/** The width the user last set, or null for the default. */
export function readShellWidths(): ShellWidths {
  return {
    rail: readWidth(RAIL_WIDTH_STORAGE_KEY),
  }
}

export function writeShellWidths(widths: ShellWidths): void {
  if (widths.rail !== null) writeStored(RAIL_WIDTH_STORAGE_KEY, String(Math.round(widths.rail)))
}
