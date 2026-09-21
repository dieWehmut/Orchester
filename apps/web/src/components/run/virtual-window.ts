/**
 * The transcript's virtual window, task U3-03 of the implementation plan.
 *
 * The window is computed from heights the transcript actually measured, never
 * from a guessed row height: a transcript that guesses drops the reader
 * somewhere else the moment the guess is wrong. Until every row has been
 * measured the honest window is all of them, because the alternative is
 * unmounting rows whose heights are unknown and moving the reading position.
 */

export interface VirtualWindowInput {
  /** The measured block size of each row, in order; `undefined` when unmeasured. */
  heights: readonly (number | undefined)[]
  scrollTop: number
  viewportHeight: number
  /** Rows mounted beyond the viewport, so a flick of the wheel lands on real rows. */
  overscan: number
}

export interface VirtualWindow {
  /** First mounted row, inclusive. */
  start: number
  /** One past the last mounted row, exclusive. */
  end: number
  /** The full scrollable height, so the scrollbar still reflects the transcript. */
  totalHeight: number
  /** Where the mounted rows begin, so they can be offset inside the scroller. */
  offsetTop: number
}

/** Every row mounted: the answer whenever layout cannot be trusted yet. */
function mountAll(heights: readonly (number | undefined)[]): VirtualWindow {
  return {
    start: 0,
    end: heights.length,
    totalHeight: heights.reduce<number>((total, height) => total + (height ?? 0), 0),
    offsetTop: 0,
  }
}

export function virtualWindow(input: VirtualWindowInput): VirtualWindow {
  const { heights, scrollTop, viewportHeight, overscan } = input

  if (heights.length === 0) return { start: 0, end: 0, totalHeight: 0, offsetTop: 0 }

  // An unmeasured row or a viewport that has not been laid out means the
  // measured heights cannot be trusted to place anything.
  const unmeasured = heights.some((height) => height === undefined)
  if (unmeasured || viewportHeight <= 0) return mountAll(heights)

  const offsets: number[] = []
  let total = 0
  for (const height of heights as readonly number[]) {
    offsets.push(total)
    total += height
  }

  const firstVisible = offsets.findIndex(
    (offset, index) => offset + (heights[index] as number) > scrollTop,
  )
  const visibleStart = firstVisible < 0 ? heights.length - 1 : firstVisible

  const viewportBottom = scrollTop + viewportHeight
  let visibleEnd = heights.length
  for (let index = visibleStart; index < heights.length; index += 1) {
    if ((offsets[index] as number) >= viewportBottom) {
      visibleEnd = index
      break
    }
  }

  const start = Math.max(0, visibleStart - overscan)
  const end = Math.min(heights.length, Math.max(visibleEnd, start + 1) + overscan)

  return { start, end, totalHeight: total, offsetTop: offsets[start] as number }
}
