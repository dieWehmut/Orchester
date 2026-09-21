import { readFileSync } from 'node:fs'
import { resolve as resolvePath } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * The contrast contract from the design spec's accessibility section.
 *
 * The spec says the alpha borders "must be verified rather than assumed", so
 * this reads the colour out of the token file and computes the ratio. A token
 * whose colour cannot be resolved fails loudly instead of being skipped,
 * because an unverifiable pair is not a passing one.
 *
 * The contract has three tiers and they are deliberately not the same bar:
 *
 * - Text a reader has to read clears 4.5:1 on every surface it can land on.
 * - Anything that has to be *seen* to be understood - a control boundary, a
 *   focus ring, an intent outline, a status fill - clears 3:1.
 * - Separators carry no meaning on their own. The design keeps its surfaces
 *   within two neutral steps and relies on a hairline to part them, so the
 *   separator roles only have to stay ordered and visible. Forcing them to
 *   3:1 would wire-frame every panel in the product.
 */

const tokens = readFileSync(resolvePath(process.cwd(), 'src/tokens.css'), 'utf8')

/** The declarations made by one selector block, nested braces included. */
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
        for (const match of tokens
          .slice(open + 1, index)
          .matchAll(/(--[a-z0-9-]+):\s*([^;]+);/g)) {
          found[match[1]!] = match[2]!.trim()
        }
        return found
      }
    }
  }
  throw new Error(`unterminated selector block: ${selector}`)
}

const root = block(':root')
const themeBlocks = {
  dark: block("[data-theme='dark']"),
  light: block("[data-theme='light']"),
} as const
const schemeBlocks: Record<string, Record<string, string>> = {}
for (const theme of ['dark', 'light'] as const) {
  for (const scheme of ['codex', 'violet', 'teal', 'rose'] as const) {
    schemeBlocks[`${theme}/${scheme}`] = block(
      `[data-theme='${theme}'][data-color-scheme='${scheme}']`,
    )
  }
}
const intensityBlocks = {
  vivid: block("[data-intensity='vivid']"),
  calm: block("[data-intensity='calm']"),
} as const

/** A cascade in file order: the last scope that declares a token wins. */
type Scope = readonly Record<string, string>[]

function scopeOf(theme: 'dark' | 'light', scheme?: string): Scope {
  return scheme
    ? [root, themeBlocks[theme], schemeBlocks[`${theme}/${scheme}`]!]
    : [root, themeBlocks[theme]]
}

function lookup(name: string, scope: Scope): string {
  // CSS resolves equal-specificity declarations in file order, and the theme,
  // scheme and intensity blocks all sit after `:root`, so the last layer that
  // declares a token is the one a browser paints.
  for (let index = scope.length - 1; index >= 0; index -= 1) {
    const value = scope[index]![name]
    if (value !== undefined) return value
  }
  throw new Error(`undefined token: ${name}`)
}

/** Resolves a token through `var()` references, or throws naming the token. */
function valueOf(name: string, scope: Scope): string {
  let current = name.startsWith('--') ? lookup(name, scope) : name
  const seen = new Set<string>()
  while (current.startsWith('var(')) {
    const referenced = current.slice(4, current.indexOf(')'))
    if (seen.has(referenced)) throw new Error(`circular token: ${referenced}`)
    seen.add(referenced)
    current = lookup(referenced, scope)
  }
  return current.trim()
}

function hexOf(name: string, scope: Scope): string {
  const value = valueOf(name, scope)
  const match = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.exec(value)
  if (!match) throw new Error(`${name} does not resolve to a hex colour: ${value}`)
  const digits = match[1]!
  return (
    digits.length === 3
      ? digits
          .split('')
          .map((digit) => digit + digit)
          .join('')
      : digits
  ).toLowerCase()
}

/**
 * The colour and alpha of a translucent token.
 *
 * Every translucent token in the file is one opaque colour at a percentage, so
 * the pair can be read straight out of the declaration: `color-mix(in oklab,
 * var(--blue-400) 13%, transparent)` and `rgb(79 191 173 / 38%)` both mean
 * "paint this colour at this alpha".
 */
function paintOf(name: string, scope: Scope): { colour: string; alpha: number } {
  const value = valueOf(name, scope)
  const mixed = /color-mix\(in oklab, var\((--[a-z0-9-]+)\) ([\d.]+)%, transparent\)/.exec(value)
  if (mixed) return { colour: hexOf(`var(${mixed[1]})`, scope), alpha: Number(mixed[2]) / 100 }
  const rgb = /rgb\((\d+) (\d+) (\d+) \/ ([\d.]+)%\)/.exec(value)
  if (rgb) {
    return {
      colour: [rgb[1], rgb[2], rgb[3]]
        .map((channel) => Number(channel).toString(16).padStart(2, '0'))
        .join(''),
      alpha: Number(rgb[4]) / 100,
    }
  }
  const opaque = /^#([0-9a-fA-F]{6})$/.exec(value)
  if (opaque) return { colour: opaque[1]!.toLowerCase(), alpha: 1 }
  throw new Error(`${name} is not a paintable colour: ${value}`)
}

function luminance(hex: string): number {
  const channels = [0, 2, 4].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255)
  const linear = channels.map((channel) =>
    channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  )
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!
}

function contrast(a: string, b: string): number {
  const la = luminance(a)
  const lb = luminance(b)
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05)
}

/** Paints a translucent colour over an opaque one, as a browser would. */
function over(background: string, paint: { colour: string; alpha: number }): string {
  return [0, 2, 4]
    .map((offset) => {
      const behind = parseInt(background.slice(offset, offset + 2), 16)
      const front = parseInt(paint.colour.slice(offset, offset + 2), 16)
      const channel = Math.round(front * paint.alpha + behind * (1 - paint.alpha))
      return channel.toString(16).padStart(2, '0')
    })
    .join('')
}

const SURFACES = ['base', 'secondary', 'tertiary', 'elevated'] as const
const TEXT_ROLES = ['emphasis', 'default', 'subtle'] as const
const SEPARATORS = ['subtle', 'default', 'emphasis'] as const
const INTENTS = ['info', 'success', 'warning', 'caution', 'danger', 'discovery'] as const

function surfacesOf(scope: Scope): Record<string, string> {
  return Object.fromEntries(
    SURFACES.map((surface) => [surface, hexOf(`--color-surface-${surface}`, scope)]),
  )
}

for (const theme of ['dark', 'light'] as const) {
  const scope = scopeOf(theme)
  const surfaces = surfacesOf(scope)

  describe(`${theme} theme contrast`, () => {
    it('resolves every surface role to a colour', () => {
      for (const surface of SURFACES) {
        expect(surfaces[surface], `--color-surface-${surface}`).toMatch(/^[0-9a-f]{6}$/)
      }
    })

    it('keeps every text role readable on every surface it can paint on', () => {
      // A text role is a promise about any surface the design lets it sit on,
      // not just its usual one, so the lightest surface is checked too.
      const failures: string[] = []
      for (const role of TEXT_ROLES) {
        const foreground = hexOf(`--color-text-${role}`, scope)
        for (const surface of SURFACES) {
          const ratio = contrast(foreground, surfaces[surface]!)
          if (ratio < 4.5) {
            failures.push(`--color-text-${role} on --color-surface-${surface}: ${ratio.toFixed(2)}:1`)
          }
        }
      }
      expect(failures).toEqual([])
    })

    it('keeps the control boundary above 3:1 on every surface it can sit on', () => {
      // This is the boundary a reader uses to find the control at all, so it is
      // measured against every surface rather than the friendliest one.
      const paint = paintOf('--color-border-control', scope)
      const failures: string[] = []
      for (const surface of SURFACES) {
        const ratio = contrast(over(surfaces[surface]!, paint), surfaces[surface]!)
        if (ratio < 3) {
          failures.push(`--color-border-control on --color-surface-${surface}: ${ratio.toFixed(2)}:1`)
        }
      }
      expect(failures).toEqual([])
    })

    it('keeps the control boundary above 3:1 on the surface its own controls fill with', () => {
      // An input fills with `--color-bg-input`, which is darker than the panel
      // behind it, so the boundary has to be checked against the fill as well.
      const fill = hexOf('--color-bg-input', scope)
      const paint = paintOf('--color-border-control', scope)
      const ratio = contrast(over(fill, paint), fill)
      expect(ratio, `--color-border-control on --color-bg-input`).toBeGreaterThanOrEqual(3)
    })

    it('keeps the control boundary stronger at rest than at hover', () => {
      // Hover has to read as a change, which means it cannot be the same value.
      const rest = paintOf('--color-border-control', scope)
      const hover = paintOf('--color-border-control-hover', scope)
      expect(hover.alpha).toBeGreaterThanOrEqual(rest.alpha)
    })

    it('keeps the focus indicator above 3:1 on every surface', () => {
      // The focus ring is the one indicator a keyboard user navigates by.
      const focus = hexOf('--color-border-focus', scope)
      const failures: string[] = []
      for (const surface of SURFACES) {
        const ratio = contrast(focus, surfaces[surface]!)
        if (ratio < 3) {
          failures.push(`--color-border-focus on --color-surface-${surface}: ${ratio.toFixed(2)}:1`)
        }
      }
      expect(failures).toEqual([])
    })

    it('keeps the separators drawn and ordered rather than held to the control bar', () => {
      // The separator roles are deliberately exempt from 3:1. They carry no
      // meaning on their own: the design separates panels by keeping surfaces
      // within two neutral steps and only parts them with a hairline, so a
      // separator loud enough to clear 3:1 would wire-frame every panel and
      // undo the "calm surfaces, loud states" principle.
      //
      // What they do owe is that they are drawn, that they stay ordered, and
      // that the strongest of them actually changes the pixels it is drawn on.
      // The boundary that *does* carry meaning is `--color-border-control`, and
      // that one is held to 3:1 above.
      const paints = SEPARATORS.map((role) => paintOf(`--color-border-${role}`, scope))
      for (const [index, paint] of paints.entries()) {
        expect(paint.alpha, `--color-border-${SEPARATORS[index]} must be drawn`).toBeGreaterThan(0)
      }
      for (let index = 1; index < paints.length; index += 1) {
        expect(
          paints[index]!.alpha,
          `--color-border-${SEPARATORS[index]} must be stronger than the step below it`,
        ).toBeGreaterThan(paints[index - 1]!.alpha)
      }
      const strongest = paints[paints.length - 1]!
      for (const surface of SURFACES) {
        const painted = over(surfaces[surface]!, strongest)
        expect(
          painted,
          `--color-border-emphasis over --color-surface-${surface} must change the pixel`,
        ).not.toBe(surfaces[surface])
      }
    })

    it('keeps every intent readable on its own tint, over every surface', () => {
      const failures: string[] = []
      for (const intent of INTENTS) {
        const foreground = hexOf(`--color-intent-${intent}-text`, scope)
        const tint = paintOf(`--color-intent-${intent}-surface`, scope)
        for (const surface of SURFACES) {
          const painted = over(surfaces[surface]!, tint)
          const ratio = contrast(foreground, painted)
          if (ratio < 4.5) {
            failures.push(
              `--color-intent-${intent}-text on its tint over --color-surface-${surface}: ${ratio.toFixed(2)}:1`,
            )
          }
        }
      }
      expect(failures).toEqual([])
    })

    it('keeps every intent readable on the plain surfaces it can label', () => {
      // Intent text also labels rows and cards that carry no tint at all.
      const failures: string[] = []
      for (const intent of INTENTS) {
        const foreground = hexOf(`--color-intent-${intent}-text`, scope)
        for (const surface of SURFACES) {
          const ratio = contrast(foreground, surfaces[surface]!)
          if (ratio < 4.5) {
            failures.push(
              `--color-intent-${intent}-text on --color-surface-${surface}: ${ratio.toFixed(2)}:1`,
            )
          }
        }
      }
      expect(failures).toEqual([])
    })

    it('keeps every intent outline above 3:1, tinted and plain', () => {
      const failures: string[] = []
      for (const intent of INTENTS) {
        const outline = hexOf(`--color-intent-${intent}-border`, scope)
        const tint = paintOf(`--color-intent-${intent}-surface`, scope)
        for (const surface of SURFACES) {
          const plain = contrast(outline, surfaces[surface]!)
          if (plain < 3) {
            failures.push(
              `--color-intent-${intent}-border on --color-surface-${surface}: ${plain.toFixed(2)}:1`,
            )
          }
          const tinted = contrast(outline, over(surfaces[surface]!, tint))
          if (tinted < 3) {
            failures.push(
              `--color-intent-${intent}-border on its own tint over --color-surface-${surface}: ${tinted.toFixed(2)}:1`,
            )
          }
        }
      }
      expect(failures).toEqual([])
    })

    it('keeps every intent fill above 3:1 as a figure against the surfaces', () => {
      // `-solid` paints a filled shape - a progress segment, a bar - rather
      // than a boundary, and a shape nobody can see is not a shape.
      const failures: string[] = []
      for (const intent of INTENTS) {
        const fill = hexOf(`--color-intent-${intent}-solid`, scope)
        for (const surface of SURFACES) {
          const ratio = contrast(fill, surfaces[surface]!)
          if (ratio < 3) {
            failures.push(`--color-intent-${intent}-solid on --color-surface-${surface}: ${ratio.toFixed(2)}:1`)
          }
        }
      }
      expect(failures).toEqual([])
    })
  })
}

describe('accent and action contrast', () => {
  const SCHEMES = ['codex', 'violet', 'teal', 'rose'] as const

  for (const theme of ['dark', 'light'] as const) {
    it(`keeps every ${theme} scheme accent readable on every surface`, () => {
      // The accent is used for links and active markers, so it is text-adjacent
      // even where it is strictly a graphic.
      const failures: string[] = []
      for (const scheme of SCHEMES) {
        const scope = scopeOf(theme, scheme)
        const accent = hexOf('--color-accent', scope)
        for (const surface of SURFACES) {
          const ratio = contrast(accent, surfacesOf(scope)[surface]!)
          if (ratio < 3) {
            failures.push(`${scheme} accent on --color-surface-${surface}: ${ratio.toFixed(2)}:1`)
          }
        }
      }
      expect(failures).toEqual([])
    })

    it(`pairs every ${theme} scheme accent with the text that sits on it`, () => {
      // A filled accent button is a text/background pair in the other direction.
      const failures: string[] = []
      for (const scheme of SCHEMES) {
        const scope = scopeOf(theme, scheme)
        const accent = hexOf('--color-accent', scope)
        const onAccent = hexOf('--color-accent-contrast', scope)
        const ratio = contrast(accent, onAccent)
        if (ratio < 4.5) failures.push(`${scheme}: ${ratio.toFixed(2)}:1`)
      }
      expect(failures).toEqual([])
    })

    it(`pairs the ${theme} inverted action with its own text`, () => {
      // Under `calm`, the primary action is an inverted neutral rather than the
      // accent, and it has to carry its label just as well.
      const scope = [...scopeOf(theme), intensityBlocks.calm]
      const fill = hexOf('--color-action-solid', scope)
      const onFill = hexOf('--color-action-contrast', scope)
      expect(contrast(fill, onFill)).toBeGreaterThanOrEqual(4.5)
    })

    it(`pairs the ${theme} danger fill with the text it is hovered into`, () => {
      // The danger button is a transparent outline that fills with the status
      // colour on hover, swapping the label to the inverse text.
      const scope = scopeOf(theme)
      const fill = hexOf('--color-status-error', scope)
      const onFill = hexOf('--color-text-inverse', scope)
      expect(contrast(fill, onFill), '--color-status-error with --color-text-inverse').toBeGreaterThanOrEqual(4.5)
    })
  }
})
