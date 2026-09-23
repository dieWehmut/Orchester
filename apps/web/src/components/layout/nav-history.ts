/**
 * Whether the app can go back or forward, read from the history it is on.
 *
 * The browser does not tell a page whether a forward entry exists, so the
 * honest signal is the one the router already keeps: vue-router writes
 * `{ back, current, forward, position }` into `history.state` for every entry
 * it pushes, and a back link is a real link only when `back` is set. Reading
 * the position alone would be a guess - position 0 means there is nothing
 * behind you, but any other position says nothing about what is ahead.
 *
 * The value is only ever used to draw a control as disabled, so both halves
 * default to `false`: a control that is drawn enabled and then does nothing is
 * worse than one that is drawn disabled and never needed.
 */

export interface NavAvailability {
  readonly back: boolean
  readonly forward: boolean
}

const UNAVAILABLE: NavAvailability = { back: false, forward: false }

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

/**
 * Reads availability out of whatever `history.state` happens to hold.
 *
 * The state is written by the router, but it is not the router's private
 * property: a page can be loaded with someone else's state, a hash history
 * writes nothing, and a memory history writes its own shape. So the reading is
 * defensive - an entry that is not in the shape this module knows leaves both
 * directions unavailable rather than announcing a direction it cannot take.
 */
export function readNavAvailability(state: unknown): NavAvailability {
  if (!isRecord(state)) return UNAVAILABLE

  // vue-router stores the location it came from and the one it went to; a
  // `null` in either place is what an entry at the edge of the stack holds.
  const back = state.back
  const forward = state.forward

  return {
    back: typeof back === 'string' && back.length > 0,
    forward: typeof forward === 'string' && forward.length > 0,
  }
}
