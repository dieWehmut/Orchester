import { describe, expect, it } from 'vitest'

import {
  PET_ROAM_MAX_CYCLES,
  PET_ROAM_PAUSE_MAX_MS,
  PET_ROAM_PAUSE_MIN_MS,
  PET_ROAM_STRIDE_PX,
  petRoamAllowed,
  petRoamPositionAt,
  petWalkAnimation,
  planPetRoamLeg,
  planPetRoamPause,
} from '../src/features/pet/pet-roam'
import { petWalkCycleMs } from '../src/features/pet/pet-animations'

/** A sequence generator, so a planned roam is the same roam every run. */
function sequence(values: readonly number[]): () => number {
  let index = 0
  return () => values[index++ % values.length]!
}

describe('pet roam planning', () => {
  it('walks a whole number of cycles and times the leg to those cycles', () => {
    const leg = planPetRoamLeg({ span: 400 }, 200, sequence([0.9]))

    expect(leg).not.toBeNull()
    expect(leg!.direction).toBe('right')
    expect(leg!.cycles).toBe(Math.min(PET_ROAM_MAX_CYCLES, Math.floor(200 / PET_ROAM_STRIDE_PX)))
    expect(Math.abs(leg!.to - leg!.from)).toBe(leg!.cycles * PET_ROAM_STRIDE_PX)
    expect(leg!.durationMs).toBe(leg!.cycles * petWalkCycleMs())
  })

  it('faces the way it walks', () => {
    expect(petWalkAnimation('right')).toBe('running-right')
    expect(petWalkAnimation('left')).toBe('running-left')
  })

  it('turns toward the room it has, not toward the wall', () => {
    // Off centre, the longer side wins outright: the companion paces the open
    // stage rather than shuffling against the wall it is standing beside.
    const nearerLeft = planPetRoamLeg({ span: 400 }, 120, sequence([0.1]))
    const nearerRight = planPetRoamLeg({ span: 400 }, 280, sequence([0.9]))

    expect(nearerLeft!.direction).toBe('right')
    expect(nearerRight!.direction).toBe('left')
  })

  it('lets the draw settle a stage it is standing in the middle of', () => {
    const leftward = planPetRoamLeg({ span: 400 }, 200, sequence([0.1]))
    const rightward = planPetRoamLeg({ span: 400 }, 200, sequence([0.6]))

    expect(leftward!.direction).toBe('left')
    expect(rightward!.direction).toBe('right')
  })

  it('turns away from a wall rather than walking into it', () => {
    const atLeftWall = planPetRoamLeg({ span: 400 }, 0, sequence([0.1]))
    const atRightWall = planPetRoamLeg({ span: 400 }, 400, sequence([0.9]))

    expect(atLeftWall!.direction).toBe('right')
    expect(atRightWall!.direction).toBe('left')
  })

  it('keeps the leg inside the stage it was given', () => {
    for (const origin of [0, 10, 55, 120, 260, 400]) {
      for (const draw of [0, 0.4, 0.9]) {
        const leg = planPetRoamLeg({ span: 400 }, origin, sequence([draw]))
        if (!leg) continue
        expect(leg.to).toBeGreaterThanOrEqual(0)
        expect(leg.to).toBeLessThanOrEqual(400)
        expect(leg.to).toBeLessThanOrEqual(leg.from + PET_ROAM_MAX_CYCLES * PET_ROAM_STRIDE_PX)
      }
    }
  })

  it('caps a step at the longest gesture the pack carries', () => {
    const leg = planPetRoamLeg({ span: 400 }, 372, sequence([0.9]))

    expect(leg).not.toBeNull()
    expect(leg!.direction).toBe('left')
    // The room behind is over nine strides, so the cap is the only thing
    // keeping one step from turning into a march across the whole stage.
    expect(leg!.cycles).toBe(PET_ROAM_MAX_CYCLES)
    expect(leg!.to).toBe(372 - PET_ROAM_MAX_CYCLES * PET_ROAM_STRIDE_PX)
  })

  it('takes the step the open side leaves room for on a short stage', () => {
    // A stride and a half of room puts the cap out of reach, so the leg is one
    // cycle: a short stage gets short steps rather than a walk longer than the
    // stage can hold.
    const span = Math.floor(PET_ROAM_STRIDE_PX * 1.5)
    const leg = planPetRoamLeg({ span }, PET_ROAM_STRIDE_PX, sequence([0.9]))

    expect(leg!.direction).toBe('left')
    expect(leg!.cycles).toBe(1)
    expect(leg!.to).toBe(0)
  })

  it('stands still when the stage is too narrow to step in', () => {
    expect(planPetRoamLeg({ span: PET_ROAM_STRIDE_PX - 1 }, 10, sequence([0.5]))).toBeNull()
    expect(planPetRoamLeg({ span: 0 }, 0, sequence([0.5]))).toBeNull()
    expect(planPetRoamLeg({ span: 20 }, 10, sequence([0.5]))).toBeNull()
  })

  it('clamps an origin that arrived from outside the stage', () => {
    const beyond = planPetRoamLeg({ span: 200 }, 900, sequence([0.9]))

    expect(beyond!.from).toBe(200)
  })

  it('leaves a wall it is standing beside instead of shuffling at it', () => {
    // One stride from the left wall the draw would have said "left" and gone
    // nowhere, so the room decides instead and the companion paces away.
    const leg = planPetRoamLeg({ span: 400 }, PET_ROAM_STRIDE_PX, sequence([0.1]))

    expect(leg).not.toBeNull()
    expect(leg!.direction).toBe('right')
    expect(leg!.cycles).toBe(PET_ROAM_MAX_CYCLES)
  })

  it('walks partway through a leg, and reports the end once it is over', () => {
    const leg = planPetRoamLeg({ span: 400 }, 200, sequence([0.9]))!

    expect(petRoamPositionAt(leg, 0)).toBe(leg.from)
    expect(petRoamPositionAt(leg, leg.durationMs / 2)).toBeCloseTo((leg.from + leg.to) / 2, 5)
    expect(petRoamPositionAt(leg, leg.durationMs)).toBe(leg.to)
    expect(petRoamPositionAt(leg, leg.durationMs * 4)).toBe(leg.to)
  })

  it('roams while the run has nothing to say and rests when it does', () => {
    expect(petRoamAllowed('idle')).toBe(true)
    expect(petRoamAllowed('review')).toBe(true)
    expect(petRoamAllowed('running')).toBe(false)
    expect(petRoamAllowed('waiting')).toBe(false)
    expect(petRoamAllowed('failed')).toBe(false)
    expect(petRoamAllowed('waving')).toBe(false)
    expect(petRoamAllowed('jumping')).toBe(false)
  })

  it('draws every rest from the authored window', () => {
    expect(planPetRoamPause(sequence([0]))).toBe(PET_ROAM_PAUSE_MIN_MS)
    expect(planPetRoamPause(sequence([1]))).toBe(PET_ROAM_PAUSE_MAX_MS)
    const middle = planPetRoamPause(sequence([0.5]))
    expect(middle).toBeGreaterThan(PET_ROAM_PAUSE_MIN_MS)
    expect(middle).toBeLessThan(PET_ROAM_PAUSE_MAX_MS)
  })

  it('reads a random that wandered out of range as the nearest end', () => {
    expect(planPetRoamPause(sequence([-3]))).toBe(PET_ROAM_PAUSE_MIN_MS)
    expect(planPetRoamPause(sequence([11]))).toBe(PET_ROAM_PAUSE_MAX_MS)
  })
})
