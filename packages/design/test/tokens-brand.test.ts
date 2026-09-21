import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const tokens = readFileSync(resolve(process.cwd(), 'src/tokens.css'), 'utf8')

/** The declarations made by one selector block. */
function block(selector: string): string {
  const start = tokens.indexOf(selector)
  if (start < 0) throw new Error(`missing selector block: ${selector}`)
  const open = tokens.indexOf('{', start)
  let depth = 0
  for (let i = open; i < tokens.length; i += 1) {
    if (tokens[i] === '{') depth += 1
    else if (tokens[i] === '}') {
      depth -= 1
      if (depth === 0) return tokens.slice(open + 1, i)
    }
  }
  throw new Error(`unterminated block: ${selector}`)
}

const dark = block("[data-theme='dark']")
const light = block("[data-theme='light']")

const PINK_STEPS = [
  '25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000',
] as const

function luminance(hex: string): number {
  const value = hex.replace('#', '')
  const channels = [0, 2, 4].map((offset) => parseInt(value.slice(offset, offset + 2), 16) / 255)
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

describe('brand pink ramp', () => {
  it('declares every step once, outside the theme blocks', () => {
    for (const step of PINK_STEPS) {
      expect(tokens, `--pink-${step}`).toContain(`--pink-${step}:`)
    }
  })

  it('declares the alpha variants the accent tokens compose with', () => {
    for (const alpha of ['25', '50', '75', '100', '200', '300']) {
      expect(tokens, `--pink-a${alpha}`).toContain(`--pink-a${alpha}:`)
    }
  })

  it('gets darker as the step number grows, so the ramp is a ramp', () => {
    const values = PINK_STEPS.map((step) => {
      const match = tokens.match(new RegExp(`--pink-${step}:\\s*(#[0-9a-fA-F]{6})`))
      if (!match) throw new Error(`missing hex for --pink-${step}`)
      return luminance(match[1]!)
    })

    for (let index = 1; index < values.length; index += 1) {
      expect(values[index], `step ${PINK_STEPS[index]}`).toBeLessThan(values[index - 1]!)
    }
  })

  it('keeps the accent readable on the surface it is painted on', () => {
    // The dark accent sits on near-black, the light accent on white. Body-text
    // contrast (4.5:1) is the contract; both clear it with room to spare.
    expect(contrast('#f472b6', '#0d0d0d')).toBeGreaterThan(4.5)
    expect(contrast('#c0267e', '#ffffff')).toBeGreaterThan(4.5)
  })

  it('points the default scheme at the pink ramp in both themes', () => {
    expect(tokens).toMatch(
      /\[data-theme='dark'\]\[data-color-scheme='rose'\][^}]*--color-accent:\s*var\(--pink-300\)/s,
    )
    expect(tokens).toMatch(
      /\[data-theme='light'\]\[data-color-scheme='rose'\][^}]*--color-accent:\s*var\(--pink-500\)/s,
    )
  })
})

describe('legacy token migration', () => {
  it('re-points the backgrounds at the L1 surface roles', () => {
    for (const scope of [dark, light]) {
      expect(scope).toMatch(/--color-bg-base:\s*var\(--color-surface-tertiary\)/)
      expect(scope).toMatch(/--color-bg-surface:\s*var\(--color-surface-base\)/)
      expect(scope).toMatch(/--color-bg-element:\s*var\(--color-surface-secondary\)/)
      expect(scope).toMatch(/--color-bg-elevated:\s*var\(--color-surface-elevated\)/)
    }
  })

  it('re-points the borders at the alpha ramp', () => {
    for (const scope of [dark, light]) {
      expect(scope).toMatch(/--color-border-base:\s*var\(--color-border-default\)/)
      expect(scope).toMatch(/--color-border-strong:\s*var\(--color-border-emphasis\)/)
    }
  })

  it('re-points text at the L1 text roles', () => {
    for (const scope of [dark, light]) {
      expect(scope).toMatch(/--color-text-primary:\s*var\(--color-text-emphasis\)/)
      expect(scope).toMatch(/--color-text-secondary:\s*var\(--color-text-default\)/)
      expect(scope).toMatch(/--color-text-tertiary:\s*var\(--color-text-subtle\)/)
    }
  })

  it('re-points the status colours at the intent matrix', () => {
    for (const scope of [dark, light]) {
      expect(scope).toMatch(/--color-status-success:\s*var\(--color-intent-success-text\)/)
      expect(scope).toMatch(/--color-status-warning:\s*var\(--color-intent-warning-text\)/)
      expect(scope).toMatch(/--color-status-error:\s*var\(--color-intent-danger-text\)/)
      expect(scope).toMatch(/--color-status-info:\s*var\(--color-intent-info-text\)/)
    }
  })

  it('leaves no blue-tinted surface behind in the theme blocks', () => {
    for (const hex of ['#0e1013', '#16191f', '#1d2129', '#242932', '#12151a', '#262b34', '#363d49']) {
      expect(dark, `dark ${hex}`).not.toContain(hex)
    }
  })
})

describe('intensity axis', () => {
  it('declares the action pair for both intensities', () => {
    expect(tokens).toMatch(/\[data-intensity='vivid'\][^}]*--color-action-solid:/s)
    expect(tokens).toMatch(/\[data-intensity='vivid'\][^}]*--color-action-contrast:/s)
    expect(tokens).toMatch(/\[data-intensity='calm'\][^}]*--color-action-solid:/s)
    expect(tokens).toMatch(/\[data-intensity='calm'\][^}]*--color-action-contrast:/s)
  })

  it('spends the accent on the primary action when vivid', () => {
    const vivid = block("[data-intensity='vivid']")
    expect(vivid).toMatch(/--color-action-solid:\s*var\(--color-accent\)/)
  })

  it('inverts the primary action, achromatically, when calm', () => {
    const calm = block("[data-intensity='calm']")
    expect(calm).toMatch(/--color-action-solid:\s*var\(--color-text-emphasis\)/)
  })

  it('defaults the document to the accent-forward treatment', () => {
    expect(tokens).toMatch(/--color-action-solid:\s*var\(--color-accent\)/)
  })
})
