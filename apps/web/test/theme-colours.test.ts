import { readFileSync } from 'node:fs'
import { resolve as resolvePath } from 'node:path'

import { describe, expect, it } from 'vitest'

import { COLOR_SCHEMES } from '@orchester/design'

import { THEME_COLOURS, themeAccent, themeAccentContrast } from '../src/components/settings/theme-colours'

/**
 * The theme colour pair the appearance table prints.
 *
 * The table cannot read the value out of the DOM: a chained custom property is
 * resolved by the browser's cascade, and jsdom - which is where this suite runs
 * - does not substitute `var()` at all. So the pair is mirrored in TypeScript
 * and this test walks the chain in `tokens.css` to prove the mirror is real.
 * A mirrored value that drifted from the stylesheet would be a table reporting
 * a colour the product does not paint.
 */

const tokens = readFileSync(
  resolvePath(process.cwd(), '../../packages/design/src/tokens.css'),
  'utf8',
)

/** The declarations of one selector block, nested braces included. */
function block(selector: string): Record<string, string> {
  const start = tokens.indexOf(selector)
  if (start < 0) throw new Error(`missing selector block: ${selector}`)
  const open = tokens.indexOf('{', start)
  let depth = 0
  for (let index = open; index < tokens.length; index += 1) {
    if (tokens[index] === '{') depth += 1
    else if (tokens[index] === '}') {
      depth -= 1
      if (depth === 0) {
        const found: Record<string, string> = {}
        for (const match of tokens.slice(open + 1, index).matchAll(/(--[a-z0-9-]+):s*([^;]+);/g)) {
          found[match[1]!] = match[2]!.trim()
        }
        return found
      }
    }
  }
  throw new Error(`unterminated selector block: ${selector}`)
}

const root = block(':root')
const themes = {
  dark: block("[data-theme='dark']"),
  light: block("[data-theme='light']"),
} as const

/**
 * One theme's accent block, which is where the hue for a scheme is declared.
 *
 * A scheme is declared once per theme, because an accent that reads well on the
 * dark base is unreadable on the light one; walking the stylesheet's own block
 * is what keeps the cards from printing a hue the product never paints.
 */
function schemeBlock(theme: 'dark' | 'light', scheme: string): Record<string, string> {
  return block(`[data-theme='${theme}'][data-color-scheme='${scheme}']`)
}

/** Follows `var()` one hop at a time, as the cascade would. */
function resolveToken(name: string, theme: Record<string, string>): string {
  let current = name
  for (let hop = 0; hop < 12; hop += 1) {
    const value = theme[current] ?? root[current]
    if (value === undefined) throw new Error(`${name} does not resolve: missing ${current}`)
    const hopMatch = /^var\((--[a-z0-9-]+)\)$/.exec(value)
    if (hopMatch === null) return value
    current = hopMatch[1]!
  }
  throw new Error(`${name} does not resolve: chain longer than 12 hops`)
}

/**
 * A hex colour as a value rather than as a spelling.
 *
 * The stylesheet writes the same white as `#fff` in one place and `#ffffff` in
 * another, and the table prints the long form the reference shows. Comparing
 * the two strings would fail on a pair that paints identically.
 */
function hexOf(value: string): string {
  const short = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(value.trim())
  if (short !== null) return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`.toLowerCase()
  return value.trim().toLowerCase()
}

describe('theme colour mirror', () => {
  it('reports the value --color-bg-base resolves to, per theme', () => {
    for (const theme of ['dark', 'light'] as const) {
      expect(hexOf(THEME_COLOURS[theme].background)).toBe(
        hexOf(resolveToken('--color-bg-base', themes[theme])),
      )
    }
  })

  it('reports the value --color-text-primary resolves to, per theme', () => {
    for (const theme of ['dark', 'light'] as const) {
      expect(hexOf(THEME_COLOURS[theme].foreground)).toBe(
        hexOf(resolveToken('--color-text-primary', themes[theme])),
      )
    }
  })

  /**
   * The accent is the one role the mirror cannot key by theme alone: the same
   * scheme is two colours, and the light variants are darkened until
   * accent-on-base clears AA. Both halves are walked out of the stylesheet's
   * own scheme blocks, so a card cannot print a hue the product never paints.
   */
  it('reports the accent each theme paints each scheme in', () => {
    for (const theme of ['dark', 'light'] as const) {
      for (const scheme of COLOR_SCHEMES) {
        const block = schemeBlock(theme, scheme)

        expect(hexOf(themeAccent(theme, scheme))).toBe(hexOf(resolveToken('--color-accent', block)))
      }
    }
  })

  it('reports the text colour that rides on that accent', () => {
    for (const theme of ['dark', 'light'] as const) {
      for (const scheme of COLOR_SCHEMES) {
        const block = schemeBlock(theme, scheme)

        expect(hexOf(themeAccentContrast(theme, scheme))).toBe(
          hexOf(resolveToken('--color-accent-contrast', block)),
        )
      }
    }
  })
})
