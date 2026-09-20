/**
 * The transcript's scroll contract.
 *
 * Kept as plain functions rather than component internals because the two rules
 * that matter — what the state flags mean, and when to follow new output — are
 * decidable without a layout, and a layout is exactly what a test cannot fake
 * faithfully.
 */

export interface ScrollBox {
  scrollTop: number
  scrollHeight: number
  clientHeight: number
}

export interface ScrollState {
  canScrollUp: boolean
  canScrollDown: boolean
  atBottom: boolean
}

/** A sub-pixel rounding margin: browsers do not land exactly on the maximum. */
const BOTTOM_EPSILON = 1

export function readScrollState(box: ScrollBox): ScrollState {
  const maximum = Math.max(0, box.scrollHeight - box.clientHeight)
  const atTop = box.scrollTop <= BOTTOM_EPSILON
  const atBottom = box.scrollTop >= maximum - BOTTOM_EPSILON
  return {
    canScrollUp: !atTop,
    canScrollDown: !atBottom,
    atBottom,
  }
}

/**
 * The unread count after one new turn arrives. Output that arrives while the
 * reader is at the bottom is already in front of them; output that arrives
 * while they are reading back is what the indicator exists to count.
 */
export function unreadAfter(count: number, decision: ScrollDecision): number {
  return decision === 'stick' ? 0 : count + 1
}

export type FadeState = 'visible' | 'hidden'

/**
 * The top fade is drawn only when there is something above the reader to fade
 * towards; a fade over the first turn would be decoration claiming there is
 * more to read.
 */
export function fadeDecision(state: ScrollState): FadeState {
  return state.canScrollUp ? 'visible' : 'hidden'
}

export type ScrollPosition = 'at-bottom' | 'reading-back'
export type ScrollTrigger = 'appended' | 'reached-bottom' | 'jump-requested'
export type ScrollDecision = 'stick' | 'hold'

/**
 * Output arriving while the reader is looking at something older must not drag
 * them to the bottom; anything else should follow the run.
 */
export function stickDecision(position: ScrollPosition, trigger: ScrollTrigger): ScrollDecision {
  if (trigger === 'reached-bottom' || trigger === 'jump-requested') return 'stick'
  return position === 'at-bottom' ? 'stick' : 'hold'
}

