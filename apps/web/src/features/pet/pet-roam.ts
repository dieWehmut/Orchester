/**
 * Where the companion walks, and when.
 *
 * The reference companion is ambient rather than seated: while its run has
 * nothing to say it pads along the strip its host opens for it, turning to face
 * the way it is going, and it holds still the moment the run has something
 * worth attending to. The decisions here are pure - a leg is a distance, a
 * direction and a beat - so the component owns only the timers and the stage it
 * measured.
 */

import { petWalkCycleMs, type PetAnimationName } from './pet-animations'

export type PetRoamDirection = 'left' | 'right'

export interface PetRoamBounds {
  /** The travel the stage offers, in CSS pixels, the sprite's own width excluded. */
  readonly span: number
}

export interface PetRoamLeg {
  readonly from: number
  readonly to: number
  readonly direction: PetRoamDirection
  /** Whole walk cycles, so the step and its travel finish together. */
  readonly cycles: number
  readonly durationMs: number
}

/**
 * How far one walk cycle carries the companion.
 *
 * A cycle is a little under a second of animation, so this is the pace of a
 * stroll rather than a bolt: the walk rows read as an amble at this speed, and
 * the sprite is moving about eighty pixels a second.
 */
export const PET_ROAM_STRIDE_PX = 72

/**
 * The longest single leg, counted in walk cycles.
 *
 * A step has to stay short enough that a run turning busy is answered promptly
 * by the pose it asks for, so the companion crosses at most a couple of
 * strides before it looks at the run again.
 */
export const PET_ROAM_MAX_CYCLES = 2

/** The beat after the companion appears, before it takes its first step. */
export const PET_ROAM_FIRST_PAUSE_MS = 1800

/** How long the companion rests between legs. */
export const PET_ROAM_PAUSE_MIN_MS = 2600
export const PET_ROAM_PAUSE_MAX_MS = 7600

/** The walk row that faces the way a leg travels. */
export function petWalkAnimation(direction: PetRoamDirection): PetAnimationName {
  return direction === 'right' ? 'running-right' : 'running-left'
}

/**
 * Where the companion stands partway through a leg.
 *
 * The CSS transition that moves the sprite runs on the same duration, so this
 * is what lets a leg be interrupted without the sprite snapping: whoever stops
 * the walk asks where it had got to and leaves it standing there.
 */
export function petRoamPositionAt(leg: PetRoamLeg, elapsedMs: number): number {
  if (leg.durationMs <= 0) return leg.to
  const progress = Math.min(Math.max(elapsedMs / leg.durationMs, 0), 1)
  return leg.from + (leg.to - leg.from) * progress
}

/**
 * Whether the companion is free to pad about.
 *
 * The run's own poses are the ones that carry a message - a busy run, an
 * approval waiting on an answer, a failure worth noticing - so walking is what
 * the companion does with the rest of its time. Idle is that rest. A review is
 * a settled run rather than a live one, so it is also free time: the review
 * pose plays once and the companion goes back to its own habits.
 */
export function petRoamAllowed(animation: PetAnimationName): boolean {
  return animation === 'idle' || animation === 'review'
}

/**
 * Plan the next leg from where the companion stands.
 *
 * `random` is injected because a roam no test can pin down is a roam no test
 * can hold: the shell passes `Math.random`, a test passes a fixed sequence. A
 * leg is always a whole number of walk cycles, and the companion does not walk
 * into a wall closer than one stride - it turns and uses the room it has, or
 * stays put when the stage is too narrow to step in at all.
 */
export function planPetRoamLeg(
  bounds: PetRoamBounds,
  origin: number,
  random: () => number = Math.random,
): PetRoamLeg | null {
  const span = Math.max(bounds.span, 0)
  const from = Math.min(Math.max(origin, 0), span)
  const roomLeft = from
  const roomRight = span - from
  const leftIsWalkable = roomLeft >= PET_ROAM_STRIDE_PX
  const rightIsWalkable = roomRight >= PET_ROAM_STRIDE_PX
  if (!leftIsWalkable && !rightIsWalkable) return null

  // The longer side wins outright, and the draw only settles a tie. A coin
  // flipped between left and right outright would send the companion into a
  // wall it had no room to walk away from - a shuffle where it had a stride -
  // and it would strand it there. Preferring the room turns a wall into a
  // place the companion leaves, which is what makes the padding read as
  // deliberate rather than as a twitch in the corner.
  const direction: PetRoamDirection =
    roomLeft === roomRight
      ? random() < 0.5
        ? 'left'
        : 'right'
      : roomRight > roomLeft
        ? 'right'
        : 'left'

  const room = direction === 'right' ? roomRight : roomLeft
  const cycles = Math.max(1, Math.min(PET_ROAM_MAX_CYCLES, Math.floor(room / PET_ROAM_STRIDE_PX)))
  const travel = cycles * PET_ROAM_STRIDE_PX
  const to = direction === 'right' ? from + travel : from - travel

  return {
    from,
    to,
    direction,
    cycles,
    durationMs: cycles * petWalkCycleMs(),
  }
}

/** A rest drawn from the authored window, so the beat varies between legs. */
export function planPetRoamPause(random: () => number = Math.random): number {
  const window = PET_ROAM_PAUSE_MAX_MS - PET_ROAM_PAUSE_MIN_MS
  const drawn = Math.min(Math.max(random(), 0), 1)
  return Math.round(PET_ROAM_PAUSE_MIN_MS + window * drawn)
}
