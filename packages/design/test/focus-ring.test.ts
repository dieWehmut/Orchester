import { readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * The focus half of the accessibility contract that lives in this package.
 *
 * Section 7 names the colour of the ring: a 2 px `--color-border-focus`
 * outline at 2 px offset, on `:focus-visible`. The ring is the one indicator a
 * keyboard user navigates by, so a component that paints it with a different
 * colour - or with a hard-coded fallback that stops following the theme - is
 * not an equivalent replacement.
 */

function source(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), relativePath), 'utf8')
}

const tokens = source('src/tokens.css')
const reset = source('src/index.css')

const COMPONENTS = readdirSync(resolve(process.cwd(), 'src/components'))
  .filter((name) => name.endsWith('.vue'))
  .map((name) => `src/components/${name}`)

/** Every top-level declaration block in a stylesheet, with its selector. */
function blocks(css: string): { selector: string; body: string }[] {
  const found: { selector: string; body: string }[] = []
  // Comments sit above the rule they explain, so they would otherwise be read
  // as part of the selector and hide the rule from an exact-name lookup.
  const stripped = css.replace(/\/\*[\s\S]*?\*\//g, '')
  for (const match of stripped.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    found.push({ selector: match[1]!.trim(), body: match[2]! })
  }
  return found
}

describe('focus ring contract', () => {
  it('paints the shared ring with the focus token', () => {
    // The reset is the ring every control inherits when it does not override.
    expect(reset).toMatch(
      /:where\(:focus-visible\)\s*\{[^}]*outline:\s*2px solid var\(--color-border-focus\)/,
    )
    expect(reset).not.toMatch(/outline:\s*2px solid var\(--color-border-focus,\s*#/)
  })

  it('paints the token-sheet ring with the focus token at the stated offset', () => {
    const ring = blocks(tokens).find((block) => block.selector === ':focus-visible')
    expect(ring).toBeTruthy()
    expect(ring!.body).toMatch(/outline:\s*2px solid var\(--color-border-focus\)/)
    expect(ring!.body).toMatch(/outline-offset:\s*2px/)
  })

  it('keeps every component ring on the focus token', () => {
    // A ring that reaches for the accent or keeps a hard-coded fallback
    // silently stops tracking the focus colour the theme defines.
    const failures: string[] = []
    for (const path of COMPONENTS) {
      for (const block of blocks(source(path))) {
        for (const match of block.body.matchAll(/outline:\s*[^;]*;/g)) {
          const declaration = match[0]
          if (!declaration.includes('solid')) continue
          if (
            !declaration.includes('var(--color-border-focus)') &&
            !declaration.includes('var(--color-border-focus,')
          ) {
            failures.push(`${path}: ${block.selector} -> ${declaration.trim()}`)
          }
        }
      }
    }
    expect(failures).toEqual([])
  })
})
