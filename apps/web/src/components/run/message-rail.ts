import type { TimelineItem } from '@orchester/ereignis'

/**
 * The message navigation rail's marks, task U3-04 of the implementation plan.
 *
 * The rail answers one question - where did I ask that? - so it lists the user
 * turns and nothing else: an assistant answer is not a destination, because
 * the transcript already shows it under the question it belongs to. The label
 * is truncated here rather than in the component so the rule has one home.
 */

const LABEL_LIMIT = 80

export interface RailMark {
  /** The timeline index the mark jumps to. */
  index: number
  /** The hover label: the question, shortened to fit the rail. */
  label: string
}

export function railMarks(timeline: readonly TimelineItem[]): RailMark[] {
  return timeline.flatMap((item, index) => {
    if (item.type !== 'message' || item.role !== 'user') return []
    const text = item.text.trim()
    const label = text.length > LABEL_LIMIT ? `${text.slice(0, LABEL_LIMIT - 1).trimEnd()}…` : text
    return [{ index, label }]
  })
}
