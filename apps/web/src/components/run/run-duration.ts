import type { RunView } from '@orchester/ereignis'

/**
 * How long a run took, in the reference's three shapes.
 *
 * The reference puts a turn's elapsed time above its answer (`用时 33m 20s`).
 * This product has one turn per run, so the run's own elapsed time is what can
 * be stated honestly, and it is computed from the event stream's own timestamps
 * - the first and last events the runtime journalled - rather than from whenever
 * this window happened to open.
 */

/** Seconds while it is quick, minutes and seconds while it is not, hours with the rest padded. */
export function formatElapsed(milliseconds: number): string {
  const total = Math.max(0, Math.round(milliseconds / 1000))
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = total % 60
  if (hours > 0) return `${hours}h ${String(minutes).padStart(2, '0')}m ${String(seconds).padStart(2, '0')}s`
  if (minutes > 0) return `${minutes}m ${seconds}s`
  return `${seconds}s`
}

/**
 * How long one answer took, or null for a row that is not a settled answer.
 *
 * The reference states this at the turn (`用时 33m 20s` above the answer), and
 * the number it states is the interval the turn occupied. Here that interval is
 * the distance from the entry before the answer to the answer's own last event -
 * the journal's own timestamps, so nothing has to be estimated: a question at
 * 21:53 answered at 22:26 took the model 33 minutes.
 *
 * Only settled answers: a row that is still arriving has not finished taking
 * anything, and a turn still in flight is the strip's business.
 */
export function answerElapsed(
  timeline: readonly { occurredAt: string | null; type?: string; role?: string; final?: boolean }[],
  index: number,
): string | null {
  const item = timeline[index]
  if (item === undefined) return null
  if (item.type !== 'message' || item.role !== 'assistant' || item.final !== true) return null
  const at = item.occurredAt === null ? Number.NaN : Date.parse(item.occurredAt)
  if (!Number.isFinite(at)) return null
  const previous = index > 0 ? timeline[index - 1]?.occurredAt ?? null : null
  const from = previous === null ? Number.NaN : Date.parse(previous)
  if (!Number.isFinite(from)) return null
  const milliseconds = at - from
  if (milliseconds < 1000) return null
  return formatElapsed(milliseconds)
}

/**
 * The run's elapsed time, or null when it has not been running for a moment yet.
 *
 * A run with one event took no measurable time, and saying "0s" beside a run in
 * flight would be a claim the stream does not support.
 */
export function runElapsed(view: RunView): string | null {
  // A row without a timestamp - the gap the journal can leave - contributes
  // nothing rather than a bogus instant.
  const stamps = view.timeline
    .map((item) => (item.occurredAt === null ? Number.NaN : Date.parse(item.occurredAt)))
    .filter((value) => Number.isFinite(value))
  if (stamps.length === 0) return null
  const started = Math.min(...stamps)
  const stop = view.stop === null ? null : Date.parse(view.stop.occurredAt)
  const ended = stop !== null && Number.isFinite(stop) ? Math.max(stop, ...stamps) : Math.max(...stamps)
  const milliseconds = ended - started
  if (milliseconds < 1000) return null
  return formatElapsed(milliseconds)
}