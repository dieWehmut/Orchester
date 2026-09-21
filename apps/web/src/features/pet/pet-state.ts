/**
 * Which animation the companion is playing, and why.
 *
 * The companion is an ambient mirror of the run, so its state is derived from
 * the run projection rather than from transport health: a run that is mid-turn
 * looks busy, a run waiting on a decision asks for one, and a finished run holds
 * a review pose. Idle is the resting state the others decay back into.
 */

import type { RunStatus } from '@orchester/ereignis'

import type { PetAnimationName } from './pet-animations'

export type PetNotificationKind = 'running' | 'waiting' | 'review' | 'failed'

export interface PetStateInput {
  readonly runStatus: RunStatus
  readonly busy: boolean
  readonly pendingApprovals: number
  readonly errorMessage: string | null
}

export interface PetState {
  readonly animation: PetAnimationName
  readonly notification: PetNotificationKind | null
}

const IDLE: PetState = { animation: 'idle', notification: null }

/** Stop reasons that mean the run ended in a state worth reacting to. */
const FAILED_STATUSES: readonly RunStatus[] = [
  'failed',
  'budget_exceeded',
  'repeated_failure',
  'interrupted_unknown_outcome',
]

const REVIEW_STATUSES: readonly RunStatus[] = ['succeeded', 'cancelled']

/**
 * Map the run projection onto the companion's animation and notification.
 *
 * Order matters: a pending approval outranks the run's own status because the
 * run cannot proceed until the user answers, and a terminal failure outranks a
 * success for the same reason a failure banner does - the last thing that
 * happened is the thing worth showing.
 */
export function petStateFor(input: PetStateInput): PetState {
  if (input.pendingApprovals > 0 || input.runStatus === 'awaiting_approval') {
    return { animation: 'waiting', notification: 'waiting' }
  }
  if (input.errorMessage !== null || FAILED_STATUSES.includes(input.runStatus)) {
    return { animation: 'failed', notification: 'failed' }
  }
  if (input.runStatus === 'running' || input.busy) {
    return { animation: 'running', notification: 'running' }
  }
  if (REVIEW_STATUSES.includes(input.runStatus)) {
    return { animation: 'review', notification: 'review' }
  }
  return IDLE
}
