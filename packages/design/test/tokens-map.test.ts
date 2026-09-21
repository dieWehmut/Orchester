import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

import { TOKEN_GROUPS, tokenNames } from '../src/tokens-map'

const tokens = readFileSync(resolve(process.cwd(), 'src/tokens.css'), 'utf8')

/**
 * The typed token index, task U0-11 of the implementation plan.
 *
 * The plan asks for the new token names to be exported as a typed map so tests
 * and tooling share one list rather than each spelling the names it happens to
 * care about. The map is only worth having if it is real: a group that names a
 * token the stylesheet does not declare is a group that would pass a test the
 * stylesheet fails.
 */

describe('token groups', () => {
  it('names every group the token architecture layers', () => {
    for (const group of [
      'neutral',
      'accents',
      'alpha',
      'intents',
      'text',
      'surfaces',
      'borders',
      'radii',
      'motion',
      'density',
      'shell',
    ] as const) {
      expect(TOKEN_GROUPS[group], group).toBeTruthy()
      expect(TOKEN_GROUPS[group].length, group).toBeGreaterThan(0)
    }
  })

  it('spells every name the way a custom property is spelled', () => {
    for (const names of Object.values(TOKEN_GROUPS)) {
      for (const name of names) {
        expect(name).toMatch(/^--[a-z0-9-]+$/)
      }
    }
  })

  it('names only tokens the stylesheet actually declares', () => {
    // A map that drifts from the stylesheet is worse than no map: it would let
    // a test pass against a name the browser never resolves.
    const missing = tokenNames().filter((name) => !tokens.includes(`${name}:`))
    expect(missing).toEqual([])
  })

  it('carries every layer wave U0 added, not only the ramps', () => {
    // The plan lists eleven foundation tasks; a map that only indexes the ramps
    // would call itself the token index while wave U0's type scales, elevation
    // and radius bases were missing from it.
    for (const [group, sample] of [
      ['typography', '--font-heading-lg-size'],
      ['typography', '--font-text-2xs-line-height'],
      ['typography', '--font-small-caps-md-weight'],
      ['elevation', '--elevation-200-geo'],
      ['elevation', '--elevation-composer-dark'],
      ['elevation', '--shadow-400'],
      ['radii', '--radius-lg-base'],
      ['radii', '--corner-radius-scale'],
      ['motion', '--cubic-enter'],
    ] as const) {
      expect(TOKEN_GROUPS[group], group).toContain(sample)
    }
  })

  it('names every step of each type scale the stylesheet declares', () => {
    const scales = {
      heading: ['xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl'],
      text: ['3xs', '2xs', 'xs', 'sm', 'md', 'lg'],
    } as const
    for (const [scale, steps] of Object.entries(scales)) {
      const declared = steps.flatMap((step) =>
        ['size', 'line-height', 'tracking', 'weight'].map(
          (property) => `--font-${scale}-${step}-${property}`,
        ),
      )
      for (const name of declared) {
        expect(TOKEN_GROUPS.typography, name).toContain(name)
      }
    }
  })

  it('flattens to one unique list, so tooling has one thing to read', () => {
    const all = tokenNames()
    expect(new Set(all).size).toBe(all.length)
    expect(all).toContain('--hit-target-min')
    expect(all).toContain('--color-intent-danger-text')
    expect(all).toContain('--density-row-height')
  })
})
