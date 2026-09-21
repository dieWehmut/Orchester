import type { MessageTimelineItem } from '@orchester/ereignis'

/**
 * Word-arrival streaming, task U3-07 of the implementation plan.
 *
 * A turn that is still arriving is marked so the transcript can contain its
 * layout: `contain: layout paint` plus `content-visibility: auto` keeps a
 * growing text node from measuring its ancestors, which is what would otherwise
 * re-run the virtual window on every word. The mark is also what the styles
 * hang off, so the rule has one home rather than a class per surface.
 */

export type ArrivalState = 'streaming' | 'settled'

export function arrivalState(item: Pick<MessageTimelineItem, 'final'>): ArrivalState {
  return item.final ? 'settled' : 'streaming'
}

/**
 * The style an arriving turn carries. `contain` is the load-bearing part: it
 * stops the growth from being observed by the transcript's own layout, so the
 * rows around it keep the boxes they already had.
 */
export const streamingContainment = {
  contain: 'layout paint',
  contentVisibility: 'auto',
} as const
