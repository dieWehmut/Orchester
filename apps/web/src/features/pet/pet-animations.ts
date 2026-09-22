/**
 * The animation tracks a v2 pack is guaranteed to have.
 *
 * These mirror the Codex companion's defaults: each app-state row holds its
 * used columns, and the track plays those frames three times before settling
 * into the idle loop. That repetition is what keeps a steady state - a long run
 * or a finished review - from looking like it stutters back to idle on every
 * short cycle.
 */

import { PET_STANDARD_ROWS, frameIndex, type PetGrid } from './pet-manifest'

export const PET_ANIMATION_NAMES = [
  'idle',
  'running-right',
  'running-left',
  'waving',
  'jumping',
  'failed',
  'waiting',
  'running',
  'review',
] as const

export type PetAnimationName = (typeof PET_ANIMATION_NAMES)[number]

export interface PetAnimationFrame {
  readonly index: number
  readonly durationMs: number
}

export interface PetAnimation {
  readonly name: PetAnimationName
  /** Frames before `loopStart` play once; the rest repeat. */
  readonly frames: readonly PetAnimationFrame[]
  readonly loopStart: number | null
  readonly fallback: PetAnimationName
}

const IDLE_TIMING: readonly (readonly [number, number])[] = [
  [0, 1680],
  [1, 660],
  [2, 660],
  [3, 840],
  [4, 840],
  [5, 1920],
]

const STATE_TIMING: Record<
  Exclude<PetAnimationName, 'idle'>,
  {
    readonly row: number
    readonly columns: number
    readonly frameMs: number
    readonly finalMs: number
  }
> = {
  'running-right': { row: 1, columns: 8, frameMs: 120, finalMs: 220 },
  'running-left': { row: 2, columns: 8, frameMs: 120, finalMs: 220 },
  waving: { row: 3, columns: 4, frameMs: 140, finalMs: 280 },
  jumping: { row: 4, columns: 5, frameMs: 140, finalMs: 280 },
  failed: { row: 5, columns: 8, frameMs: 140, finalMs: 240 },
  waiting: { row: 6, columns: 6, frameMs: 150, finalMs: 260 },
  running: { row: 7, columns: 6, frameMs: 120, finalMs: 220 },
  review: { row: 8, columns: 6, frameMs: 150, finalMs: 280 },
}

/**
 * How many times a state cycle plays before the track hands back to idle.
 */
export const PET_STATE_REPEATS = 3

/** One pass through a state row, in the pack's own milliseconds. */
export function petStateCycleMs(name: Exclude<PetAnimationName, 'idle'>): number {
  const timing = STATE_TIMING[name]
  return (timing.columns - 1) * timing.frameMs + timing.finalMs
}

/** One walk cycle, shared by the two walk rows because the pack authors them alike. */
export function petWalkCycleMs(): number {
  return petStateCycleMs('running-right')
}

/**
 * How long a pose holds before its track settles into idle.
 *
 * Idle is the resting state and holds nothing, so it reports no gesture at all:
 * a caller waiting for the companion to finish saying something has nothing to
 * wait for once it is resting.
 */
export function petPoseHoldMs(name: PetAnimationName): number {
  if (name === 'idle') return 0
  return PET_STATE_REPEATS * petStateCycleMs(name)
}

/**
 * One walk row as a looping track, rather than a state track that hands off.
 *
 * A leg is shorter than the three cycles a state track carries, so the walk
 * cannot use those frames: it needs the step to repeat for as long as the leg
 * lasts and to stop the moment the direction clears.
 */
export function createPetWalkTrack(
  grid: PetGrid,
  name: Extract<PetAnimationName, 'running-left' | 'running-right'>,
): PetAnimation | null {
  const frames = stateFrames(grid, name)
  if (frames.length === 0) return null
  return { name, frames, loopStart: 0, fallback: 'idle' }
}

function idleFrames(grid: PetGrid): PetAnimationFrame[] {
  return IDLE_TIMING.flatMap(([column, durationMs]) => {
    const index = frameIndex(grid, column, 0)
    return index === null ? [] : [{ index, durationMs }]
  })
}

function stateFrames(grid: PetGrid, name: Exclude<PetAnimationName, 'idle'>): PetAnimationFrame[] {
  const timing = STATE_TIMING[name]
  const cycle: PetAnimationFrame[] = []
  for (let column = 0; column < timing.columns; column += 1) {
    const index = frameIndex(grid, column, timing.row)
    if (index === null) continue
    const last = column === timing.columns - 1
    cycle.push({ index, durationMs: last ? timing.finalMs : timing.frameMs })
  }
  return cycle
}

/**
 * Build every track for a grid, skipping any row the pack does not carry.
 *
 * A standard (pre-v2) pack has a truncated grid, so tracks whose row is missing
 * are simply absent from the map and `resolveAnimation` falls back to idle.
 */
export function createPetAnimations(grid: PetGrid): Map<PetAnimationName, PetAnimation> {
  const tracks = new Map<PetAnimationName, PetAnimation>()
  const idle = idleFrames(grid)
  if (idle.length > 0) {
    tracks.set('idle', { name: 'idle', frames: idle, loopStart: 0, fallback: 'idle' })
  }
  if (grid.rows >= PET_STANDARD_ROWS) {
    for (const name of PET_ANIMATION_NAMES) {
      if (name === 'idle') continue
      const cycle = stateFrames(grid, name)
      if (cycle.length === 0) continue
      const frames = [...cycle, ...cycle, ...cycle, ...idle]
      tracks.set(name, {
        name,
        frames,
        loopStart: cycle.length * PET_STATE_REPEATS,
        fallback: 'idle',
      })
    }
  }
  return tracks
}

export function resolveAnimation(
  tracks: ReadonlyMap<PetAnimationName, PetAnimation>,
  name: PetAnimationName,
): PetAnimation | null {
  return tracks.get(name) ?? tracks.get('idle') ?? null
}

function frameAtElapsed(animation: PetAnimation, elapsedMs: number): PetAnimationFrame | null {
  let remaining = elapsedMs
  for (const frame of animation.frames) {
    const span = Math.max(frame.durationMs, 1)
    if (remaining < span) return frame
    remaining -= span
  }
  return animation.frames.at(-1) ?? null
}

/**
 * The frame visible at `elapsedMs`, honouring loop semantics.
 *
 * Tracks with a `loopStart` play their prefix once and then repeat only the tail,
 * which ends in the idle cycle, so the companion always returns to a resting
 * pose instead of freezing on its last gesture.
 */
export function animationFrameAt(
  animation: PetAnimation,
  elapsedMs: number,
): PetAnimationFrame | null {
  const first = animation.frames.at(0)
  if (!first) return null
  if (animation.frames.length === 1) return first

  const clamped = Math.max(elapsedMs, 0)
  const loopStart = animation.loopStart
  if (loopStart !== null && loopStart < animation.frames.length) {
    const prefix = animation.frames.slice(0, loopStart)
    const tail = animation.frames.slice(loopStart)
    const prefixMs = prefix.reduce((sum, frame) => sum + frame.durationMs, 0)
    const tailMs = tail.reduce((sum, frame) => sum + frame.durationMs, 0)
    if (clamped >= prefixMs && tailMs > 0) {
      const offset = (clamped - prefixMs) % tailMs
      return frameAtElapsed({ ...animation, frames: tail }, offset)
    }
    return frameAtElapsed(animation, clamped)
  }
  const total = animation.frames.reduce((sum, frame) => sum + frame.durationMs, 0)
  if (clamped >= total) return animation.frames.at(-1) ?? null
  return frameAtElapsed(animation, clamped)
}

/** Milliseconds until the visible frame changes, or null when the track settles. */
export function animationFrameDelay(animation: PetAnimation, elapsedMs: number): number | null {
  const first = animation.frames.at(0)
  if (!first || animation.frames.length === 1) return null
  const clamped = Math.max(elapsedMs, 0)
  const loopStart = animation.loopStart
  const loops = loopStart !== null && loopStart < animation.frames.length
  const frames = loops ? animation.frames.slice(loopStart) : animation.frames
  const prefixMs = loops
    ? animation.frames.slice(0, loopStart).reduce((sum, frame) => sum + frame.durationMs, 0)
    : 0
  const windowMs = frames.reduce((sum, frame) => sum + frame.durationMs, 0)
  if (windowMs <= 0) return null
  if (!loops) {
    const settledAt = prefixMs + windowMs
    if (clamped >= settledAt) return null
  }
  const offset = clamped < prefixMs ? clamped : (clamped - prefixMs) % windowMs
  let remaining = offset
  for (const frame of frames) {
    const span = Math.max(frame.durationMs, 1)
    if (remaining < span) return span - remaining
    remaining -= span
  }
  return null
}
