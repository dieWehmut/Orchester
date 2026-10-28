import { describe, expect, it } from 'vitest'

import { compactAge, spelledAge } from '../src/lib/relative-time'

/**
 * The sidebar's trailing time.
 *
 * The reference draws `4h` on the row, not `4 hr. ago`, and `25 分` rather than
 * `25分钟前`. `Intl.RelativeTimeFormat` cannot shorten that far, so the units are
 * spelled in the module and the tests pin the width the column depends on.
 */

const NOW = 1_700_000_000_000

function ago(seconds: number): number {
  return Math.floor(NOW / 1000) - seconds
}

describe('compactAge', () => {
  it('states minutes, hours and days in the reference width', () => {
    expect(compactAge(ago(25 * 60), 'en', NOW)).toBe('25m')
    expect(compactAge(ago(4 * 3_600), 'en', NOW)).toBe('4h')
    expect(compactAge(ago(3 * 86_400), 'en', NOW)).toBe('3d')
  })

  it('uses the locale unit rather than the English one', () => {
    expect(compactAge(ago(25 * 60), 'zh-CN', NOW)).toBe('25 分')
    expect(compactAge(ago(4 * 3_600), 'zh-CN', NOW)).toBe('4 小时')
    expect(compactAge(ago(4 * 3_600), 'zh-TW', NOW)).toBe('4 小時')
  })

  it('reads "now" rather than "0m" for a run that just happened', () => {
    expect(compactAge(ago(5), 'en', NOW)).toBe('now')
    expect(compactAge(ago(5), 'zh-CN', NOW)).toBe('刚刚')
  })

  it('falls back to a date once the run is a week old', () => {
    const old = compactAge(ago(30 * 86_400), 'en', NOW)
    expect(old).not.toContain('d')
    expect(old).toMatch(/[A-Za-z]{3}/)
  })

  it('never states a negative age for a clock that ran backwards', () => {
    expect(compactAge(ago(-500), 'en', NOW)).toBe('now')
  })
})

describe('spelledAge', () => {
  it('keeps the full sentence the row hides', () => {
    expect(spelledAge(ago(4 * 3_600), 'en', NOW)).toContain('hour')
    expect(spelledAge(ago(4 * 3_600), 'zh-CN', NOW)).toContain('小时')
  })
})
