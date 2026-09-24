/**
 * The day a row belongs to, and the label the reference puts above the first
 * row of each day.
 *
 * The reference draws a transcript as a single stream and marks where the stream
 * crossed midnight (`9月16日周三 21:53`), so a reader who comes back to a long run
 * can see how much of it happened when. The label is the day and the time of the
 * first row of that day, formatted by the platform rather than by a pattern in
 * this file, so a reader's own convention decides what it looks like.
 */

/** The local calendar day of an instant, as a key two instants can be compared by. */
export function dayKey(occurredAt: string | null | undefined): string | null {
  if (occurredAt === null || occurredAt === undefined) return null
  const date = new Date(occurredAt)
  if (Number.isNaN(date.getTime())) return null
  const month = `${date.getMonth() + 1}`.padStart(2, '0')
  const day = `${date.getDate()}`.padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

/**
 * The day and the time of one instant.
 *
 * The weekday is asked for even though the date already contains it: a stream
 * that crossed a week is easier to read when each mark says which weekday it
 * was, and that is what the reference's own mark says.
 */
export function dayLabel(occurredAt: string | null | undefined): string {
  if (occurredAt === null || occurredAt === undefined) return ''
  const date = new Date(occurredAt)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

/**
 * Whether the row at `index` is the first row of its day.
 *
 * Asked of the whole transcript rather than of the rows on screen: the list is
 * virtualised, and a mark computed from the mounted window would appear again
 * every time the window moved. A row the journal left without a time - a gap -
 * opens nothing, and a row that follows one opens its day, because there is no
 * day to have been in.
 */
export function startsDay(
  timeline: readonly { occurredAt: string | null }[],
  index: number,
): boolean {
  const item = timeline[index]
  if (item === undefined) return false
  const key = dayKey(item.occurredAt)
  // A row the journal left without a time opens nothing: it has no day to name.
  if (key === null) return false
  if (index === 0) return true
  // The day is compared against the last row that had one, so a gap between two
  // rows of the same day does not make the day look new again.
  for (let at = index - 1; at >= 0; at -= 1) {
    const previous = dayKey(timeline[at]?.occurredAt ?? null)
    if (previous !== null) return previous !== key
  }
  return true
}