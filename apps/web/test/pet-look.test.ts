import { describe, expect, it } from 'vitest'

import {
  PET_LOOK_DEADZONE_PX,
  PET_LOOK_DIRECTIONS,
  lookDirectionFor,
  petLookFor,
} from '../src/features/pet/pet-look'
import { parsePetManifest, petGridFor } from '../src/features/pet/pet-manifest'

const manifest = parsePetManifest({
  id: 'xiaoxuan',
  displayName: '小萱',
  spritesheetPath: 'spritesheet.webp',
  spriteVersionNumber: 2,
})!
const grid = petGridFor(manifest)

describe('pet look directions', () => {
  it('starts at up and walks the sixteen poses clockwise', () => {
    expect(lookDirectionFor(0, -100)).toBe(0)
    expect(lookDirectionFor(100, 0)).toBe(4)
    expect(lookDirectionFor(0, 100)).toBe(8)
    expect(lookDirectionFor(-100, 0)).toBe(12)
  })

  it('keeps neighbouring quadrants on the expected sides', () => {
    expect(lookDirectionFor(100, -100)).toBe(2)
    expect(lookDirectionFor(100, 100)).toBe(6)
    expect(lookDirectionFor(-100, 100)).toBe(10)
    expect(lookDirectionFor(-100, -100)).toBe(14)
    expect(PET_LOOK_DIRECTIONS).toBe(16)
  })

  it('addresses rows 9 and 10 for the two look halves', () => {
    const upward = petLookFor(grid, 0, -100)
    const rightward = petLookFor(grid, 100, 0)
    const downward = petLookFor(grid, 0, 100)
    const leftward = petLookFor(grid, -100, 0)

    expect(upward).toEqual({ direction: 0, frame: 72 })
    expect(rightward).toEqual({ direction: 4, frame: 76 })
    expect(downward).toEqual({ direction: 8, frame: 80 })
    expect(leftward).toEqual({ direction: 12, frame: 84 })
  })

  it('returns to idle inside the pointer deadzone', () => {
    expect(petLookFor(grid, 0, 0)).toEqual({ direction: null, frame: null })
    expect(petLookFor(grid, PET_LOOK_DEADZONE_PX - 1, 0)).toEqual({
      direction: null,
      frame: null,
    })
    expect(petLookFor(grid, PET_LOOK_DEADZONE_PX, 0).direction).toBe(4)
  })

  it('has no look frames for a standard pack', () => {
    const standard = petGridFor({ ...manifest, spriteVersionNumber: 1 })

    expect(petLookFor(standard, 0, -100)).toEqual({ direction: null, frame: null })
  })
})
