import { describe, expect, it } from 'vitest'

import {
  PET_ATLAS_HEIGHT,
  PET_ATLAS_WIDTH,
  frameIndex,
  frameOffset,
  parsePetManifest,
  petGridFor,
  resolveSpritesheetUrl,
} from '../src/features/pet/pet-manifest'

const XIAOXUAN = {
  id: 'xiaoxuan',
  displayName: '小萱',
  description: 'A poised silver-white bear-eared chibi companion.',
  spriteVersionNumber: 2,
  spritesheetPath: 'spritesheet.webp',
}

describe('pet manifest', () => {
  it('promotes a v2 pack to the 8x11 grid with its look rows', () => {
    const manifest = parsePetManifest(XIAOXUAN)

    expect(manifest).not.toBeNull()
    const grid = petGridFor(manifest!)

    expect(grid).toEqual({
      frameWidth: 192,
      frameHeight: 208,
      columns: 8,
      rows: 11,
      frameCount: 88,
    })
    expect(PET_ATLAS_WIDTH).toBe(1536)
    expect(PET_ATLAS_HEIGHT).toBe(2288)
  })

  it('keeps a standard pack on the 8x9 grid', () => {
    const grid = petGridFor({ ...XIAOXUAN, spriteVersionNumber: 1 })

    expect(grid.rows).toBe(9)
    expect(grid.frameCount).toBe(72)
  })

  it('rejects a manifest that is missing its identity or atlas', () => {
    expect(parsePetManifest(null)).toBeNull()
    expect(parsePetManifest({ displayName: 'Nameless', spritesheetPath: 'a.webp' })).toBeNull()
    expect(parsePetManifest({ id: 'a', displayName: 'A' })).toBeNull()
    expect(parsePetManifest({ id: '  ', displayName: 'A', spritesheetPath: 'a.webp' })).toBeNull()
  })

  it('defaults an unstated sprite version to the standard grid', () => {
    const manifest = parsePetManifest({
      id: 'legacy',
      displayName: 'Legacy',
      spritesheetPath: 'spritesheet.webp',
    })

    expect(manifest?.spriteVersionNumber).toBe(1)
    expect(petGridFor(manifest!).rows).toBe(9)
  })

  it('addresses cells row-major and refuses cells outside the grid', () => {
    const grid = petGridFor(parsePetManifest(XIAOXUAN)!)

    expect(frameIndex(grid, 0, 0)).toBe(0)
    expect(frameIndex(grid, 7, 0)).toBe(7)
    expect(frameIndex(grid, 0, 1)).toBe(8)
    expect(frameIndex(grid, 0, 9)).toBe(72)
    expect(frameIndex(grid, 7, 10)).toBe(87)
    expect(frameIndex(grid, 8, 0)).toBeNull()
    expect(frameIndex(grid, 0, 11)).toBeNull()
    expect(frameIndex(grid, -1, 0)).toBeNull()
  })

  it('maps frames to the sprite offsets a background sprite needs', () => {
    const grid = petGridFor(parsePetManifest(XIAOXUAN)!)

    expect(frameOffset(grid, 0)).toEqual({ x: 0, y: 0 })
    expect(frameOffset(grid, 7)).toEqual({ x: 100, y: 0 })
    expect(frameOffset(grid, 87)).toEqual({ x: 100, y: 100 })
  })

  it('joins the manifest spritesheet to its pack directory', () => {
    expect(resolveSpritesheetUrl('/pets/xiaoxuan', 'spritesheet.webp')).toBe(
      '/pets/xiaoxuan/spritesheet.webp',
    )
    expect(resolveSpritesheetUrl('/pets/xiaoxuan/', 'spritesheet.webp')).toBe(
      '/pets/xiaoxuan/spritesheet.webp',
    )
    expect(resolveSpritesheetUrl('/pets/xiaoxuan', '/spritesheet.webp')).toBe(
      '/pets/xiaoxuan/spritesheet.webp',
    )
  })
})
