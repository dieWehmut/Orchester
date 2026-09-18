/**
 * The v2 pet pack contract, as the Codex companion defines it.
 *
 * A pet is a folder holding one manifest and one atlas. `spriteVersionNumber: 2`
 * is what promotes the atlas from the 8x9 standard grid to the 8x11 one whose
 * last two rows carry the sixteen clockwise look directions, so the grid is
 * derived from that number rather than configured separately: a v2 pack cannot
 * silently disagree with the rows the renderer is about to address.
 */

export const PET_SPRITE_VERSION = 2

export const PET_FRAME_WIDTH = 192
export const PET_FRAME_HEIGHT = 208
export const PET_COLUMNS = 8
export const PET_STANDARD_ROWS = 9
export const PET_V2_ROWS = 11
export const PET_LOOK_ROWS = 2

export const PET_ATLAS_WIDTH = PET_FRAME_WIDTH * PET_COLUMNS
export const PET_ATLAS_HEIGHT = PET_FRAME_HEIGHT * PET_V2_ROWS

export interface PetManifest {
  readonly id: string
  readonly displayName: string
  readonly description: string
  readonly spriteVersionNumber: number
  readonly spritesheetPath: string
}

export interface PetGrid {
  readonly frameWidth: number
  readonly frameHeight: number
  readonly columns: number
  readonly rows: number
  readonly frameCount: number
}

export function petGridFor(manifest: PetManifest): PetGrid {
  const rows = manifest.spriteVersionNumber >= PET_SPRITE_VERSION ? PET_V2_ROWS : PET_STANDARD_ROWS
  return {
    frameWidth: PET_FRAME_WIDTH,
    frameHeight: PET_FRAME_HEIGHT,
    columns: PET_COLUMNS,
    rows,
    frameCount: PET_COLUMNS * rows,
  }
}

/**
 * The frame index a grid cell maps to, row-major.
 *
 * Returns null for a cell outside the grid so callers can keep addressing
 * rows 9-10 of a pack that turned out to be standard rather than v2.
 */
export function frameIndex(grid: PetGrid, column: number, row: number): number | null {
  if (column < 0 || column >= grid.columns) return null
  if (row < 0 || row >= grid.rows) return null
  return row * grid.columns + column
}

/** Percent offsets for a CSS background sprite cropped from the atlas. */
export function frameOffset(grid: PetGrid, index: number): { x: number; y: number } {
  const column = index % grid.columns
  const row = Math.floor(index / grid.columns)
  return {
    x: grid.columns === 1 ? 0 : (column / (grid.columns - 1)) * 100,
    y: grid.rows === 1 ? 0 : (row / (grid.rows - 1)) * 100,
  }
}

function readString(source: Record<string, unknown>, key: string): string | null {
  const value = source[key]
  if (typeof value !== 'string') return null
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : null
}

/** Parse a pet manifest defensively; a malformed pack must not break the shell. */
export function parsePetManifest(source: unknown): PetManifest | null {
  if (typeof source !== 'object' || source === null) return null
  const record = source as Record<string, unknown>
  const id = readString(record, 'id')
  const displayName = readString(record, 'displayName')
  const spritesheetPath = readString(record, 'spritesheetPath')
  if (!id || !displayName || !spritesheetPath) return null
  const version = record.spriteVersionNumber
  const spriteVersionNumber = typeof version === 'number' && Number.isFinite(version) ? version : 1
  return {
    id,
    displayName,
    description: readString(record, 'description') ?? '',
    spriteVersionNumber,
    spritesheetPath,
  }
}

/** Join a manifest-relative spritesheet name to the pack directory URL. */
export function resolveSpritesheetUrl(packUrl: string, spritesheetPath: string): string {
  const base = packUrl.endsWith('/') ? packUrl : packUrl + '/'
  return base + spritesheetPath.replace(/^\/+/, '')
}
