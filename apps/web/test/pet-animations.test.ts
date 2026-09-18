import { describe, expect, it } from 'vitest'

import {
  PET_ANIMATION_NAMES,
  animationFrameAt,
  animationFrameDelay,
  createPetAnimations,
  resolveAnimation,
} from '../src/features/pet/pet-animations'
import { parsePetManifest, petGridFor } from '../src/features/pet/pet-manifest'

const manifest = parsePetManifest({
  id: 'xiaoxuan',
  displayName: '小萱',
  spritesheetPath: 'spritesheet.webp',
  spriteVersionNumber: 2,
})!
const grid = petGridFor(manifest)
const tracks = createPetAnimations(grid)

function frameIndices(frames: readonly { index: number }[]): number[] {
  return frames.map((frame) => frame.index)
}

describe('pet animations', () => {
  it('builds every standard track for a v2 pack', () => {
    expect([...tracks.keys()]).toEqual([...PET_ANIMATION_NAMES])
  })

  it('walks the idle row with the documented hold durations', () => {
    const idle = tracks.get('idle')!

    expect(frameIndices(idle.frames)).toEqual([0, 1, 2, 3, 4, 5])
    expect(idle.frames.map((frame) => frame.durationMs)).toEqual([1680, 660, 660, 840, 840, 1920])
    expect(idle.loopStart).toBe(0)
  })

  it('plays a state row three times before handing off to idle', () => {
    const running = tracks.get('running')!

    expect(frameIndices(running.frames.slice(0, 18))).toEqual([
      56, 57, 58, 59, 60, 61, 56, 57, 58, 59, 60, 61, 56, 57, 58, 59, 60, 61,
    ])
    expect(frameIndices(running.frames.slice(18))).toEqual([0, 1, 2, 3, 4, 5])
    expect(running.loopStart).toBe(18)
  })

  it('uses the row and column budget each state declares', () => {
    expect(frameIndices(tracks.get('waving')!.frames.slice(0, 4))).toEqual([24, 25, 26, 27])
    expect(frameIndices(tracks.get('jumping')!.frames.slice(0, 5))).toEqual([32, 33, 34, 35, 36])
    expect(frameIndices(tracks.get('waiting')!.frames.slice(0, 6))).toEqual([48, 49, 50, 51, 52, 53])
    expect(frameIndices(tracks.get('review')!.frames.slice(0, 6))).toEqual([64, 65, 66, 67, 68, 69])
  })

  it('holds the final frame of each state cycle longer than the middle ones', () => {
    const failed = tracks.get('failed')!
    const cycle = failed.frames.slice(0, 8)

    expect(cycle.at(-1)?.durationMs).toBe(240)
    expect(cycle[0]?.durationMs).toBe(140)
  })

  it('keeps every track for a standard pack, which has all nine rows', () => {
    const standard = createPetAnimations(petGridFor({ ...manifest, spriteVersionNumber: 1 }))

    expect([...standard.keys()]).toEqual([...PET_ANIMATION_NAMES])
    expect(standard.get('idle')?.frames.map((frame) => frame.index)).toEqual([0, 1, 2, 3, 4, 5])
  })

  it('omits the state tracks when the grid cannot carry their rows', () => {
    const truncated = createPetAnimations({ ...grid, rows: 1, frameCount: 8 })

    expect([...truncated.keys()]).toEqual(['idle'])
  })

  it('resolves a known track, an unknown track and a missing pack', () => {
    expect(resolveAnimation(tracks, 'review')?.name).toBe('review')
    const empty = new Map()
    expect(resolveAnimation(empty, 'review')).toBeNull()
  })
})

describe('pet animation timing', () => {
  const idle = tracks.get('idle')!

  it('reports the frame visible at an elapsed time', () => {
    expect(animationFrameAt(idle, 0)?.index).toBe(0)
    expect(animationFrameAt(idle, 1679)?.index).toBe(0)
    expect(animationFrameAt(idle, 1680)?.index).toBe(1)
    expect(animationFrameAt(idle, 3000)?.index).toBe(3)
  })

  it('loops the idle track instead of settling on its last frame', () => {
    const total = idle.frames.reduce((sum, frame) => sum + frame.durationMs, 0)

    expect(animationFrameAt(idle, total)?.index).toBe(0)
    expect(animationFrameAt(idle, total + 1680)?.index).toBe(1)
  })

  it('repeats only the tail of a state track once its prefix has played', () => {
    const running = tracks.get('running')!
    const prefixMs = running.frames
      .slice(0, running.loopStart!)
      .reduce((sum, frame) => sum + frame.durationMs, 0)
    const tailMs = running.frames
      .slice(running.loopStart!)
      .reduce((sum, frame) => sum + frame.durationMs, 0)

    expect(animationFrameAt(running, 0)?.index).toBe(56)
    expect(animationFrameAt(running, prefixMs)?.index).toBe(0)
    expect(animationFrameAt(running, prefixMs + tailMs)?.index).toBe(0)
    expect(animationFrameAt(running, prefixMs + tailMs + 1680)?.index).toBe(1)
  })

  it('advances the delay until the next frame change', () => {
    expect(animationFrameDelay(idle, 0)).toBe(1680)
    expect(animationFrameDelay(idle, 1680)).toBe(660)
    expect(animationFrameDelay(idle, 2339)).toBe(1)
  })
})
