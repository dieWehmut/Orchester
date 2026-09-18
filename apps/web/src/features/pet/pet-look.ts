/**
 * Pointer-driven look directions.
 *
 * Rows 9 and 10 hold sixteen poses in fixed clockwise order starting at 000
 * degrees, which the pack defines as looking up at twelve o'clock. The front
 * pose is the deadzone: pointing straight at the companion is not a direction,
 * so it hands back to the idle animation instead of freezing on a stare.
 */

import { PET_COLUMNS, PET_LOOK_ROWS, frameIndex, type PetGrid } from './pet-manifest'

export const PET_LOOK_DIRECTIONS = 16
export const PET_LOOK_STEP_DEGREES = 360 / PET_LOOK_DIRECTIONS

/** Pointer offsets inside this radius read as facing the viewer. */
export const PET_LOOK_DEADZONE_PX = 18

export interface PetLook {
  /** Clockwise index starting at 0 = up, or null inside the deadzone. */
  readonly direction: number | null
  /** Frame index in rows 9-10, or null when the pack has no look rows. */
  readonly frame: number | null
}

const NEUTRAL: PetLook = { direction: null, frame: null }

/**
 * Quantise a pointer offset into a clockwise direction index.
 *
 * Zero degrees is up and angles grow clockwise, so an offset directly below the
 * companion is 180 degrees.
 */
export function lookDirectionFor(dx: number, dy: number): number {
  const degrees = (Math.atan2(dx, -dy) * 180) / Math.PI
  const normalized = (degrees + 360) % 360
  return Math.round(normalized / PET_LOOK_STEP_DEGREES) % PET_LOOK_DIRECTIONS
}

/** Resolve a pointer offset against the companion to a look frame. */
export function petLookFor(
  grid: PetGrid,
  dx: number,
  dy: number,
  deadzone = PET_LOOK_DEADZONE_PX,
): PetLook {
  if (grid.rows < PET_LOOK_ROWS + 1) return NEUTRAL
  if (Math.hypot(dx, dy) < deadzone) return NEUTRAL
  const direction = lookDirectionFor(dx, dy)
  const row = 9 + Math.floor(direction / PET_COLUMNS)
  const column = direction % PET_COLUMNS
  const frame = frameIndex(grid, column, row)
  return frame === null ? NEUTRAL : { direction, frame }
}
