/**
 * How long ago a run happened, in the width of a sidebar row.
 *
 * The reference's list states a time on the row's trailing edge and nothing
 * else: `4h` in English, `25 分` in Chinese. `Intl.RelativeTimeFormat` cannot
 * produce that - its shortest style still reads `4 hr. ago` - so the units are
 * spelled here, per locale, and the number is the only thing formatted.
 *
 * A run older than a week loses its relative form entirely: "2 周" says less
 * than the date does once the reader is looking that far back.
 */

type Unit = 'minute' | 'hour' | 'day'

const UNIT_LABELS = {
  en: { minute: 'm', hour: 'h', day: 'd' },
  'zh-CN': { minute: ' 分', hour: ' 小时', day: ' 天' },
  'zh-TW': { minute: ' 分', hour: ' 小時', day: ' 天' },
} satisfies Record<string, Record<Unit, string>>

function unitLabels(locale: string): Record<Unit, string> {
  if (locale.toLowerCase().startsWith('zh-tw')) return UNIT_LABELS['zh-TW']
  if (locale.toLowerCase().startsWith('zh')) return UNIT_LABELS['zh-CN']
  return UNIT_LABELS.en
}

/** The steps a row will ever show, largest first, in seconds. */
const STEPS: ReadonlyArray<{ unit: Unit; seconds: number }> = [
  { unit: 'day', seconds: 86_400 },
  { unit: 'hour', seconds: 3_600 },
  { unit: 'minute', seconds: 60 },
]

/**
 * A compact age for a row, or the date once it is a week old.
 *
 * `now` is a parameter so the tests can state a clock rather than race one.
 */
export function compactAge(
  unixSeconds: number,
  locale: string,
  now: number = Date.now(),
): string {
  const elapsed = Math.max(0, Math.floor(now / 1000) - unixSeconds)
  if (elapsed < 60) return locale.toLowerCase().startsWith('zh') ? '刚刚' : 'now'

  for (const step of STEPS) {
    if (elapsed >= step.seconds) {
      const value = Math.floor(elapsed / step.seconds)
      if (step.unit === 'day' && value >= 7) break
      return `${value}${unitLabels(locale)[step.unit]}`
    }
  }

  return new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric' }).format(
    new Date(unixSeconds * 1000),
  )
}

/** The same age, spelled out, for the tooltip and the screen reader. */
export function spelledAge(
  unixSeconds: number,
  locale: string,
  now: number = Date.now(),
): string {
  const elapsed = Math.max(0, Math.floor(now / 1000) - unixSeconds)
  const format = (value: number, unit: Unit | 'second' | 'week') =>
    new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }).format(-value, unit)

  if (elapsed < 60) return format(elapsed, 'second')
  if (elapsed < 3_600) return format(Math.floor(elapsed / 60), 'minute')
  if (elapsed < 86_400) return format(Math.floor(elapsed / 3_600), 'hour')
  if (elapsed < 604_800) return format(Math.floor(elapsed / 86_400), 'day')
  return format(Math.floor(elapsed / 604_800), 'week')
}
