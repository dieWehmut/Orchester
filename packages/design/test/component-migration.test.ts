import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * The migration sweep, wave U8 of the implementation plan.
 *
 * Two rules are what "the component layer may not reach down" means, and both
 * are checkable from the source: a component paints with an intent or app role
 * rather than a raw ramp step, and a component measures with a token rather
 * than a hand-picked number. The exceptions are the ones the design actually
 * asks for - a hairline is a hairline, and a preview that has to draw a
 * different theme's paint cannot use this theme's tokens to do it.
 *
 * The point of pinning it here rather than sweeping once is that the sweep is
 * only worth doing if it stays done: the next component added after this wave
 * has to pass the same two rules.
 */

/** The repository root: this package sits two levels under it. */
const ROOT = resolve(process.cwd(), '..', '..')

function source(relativePath: string): string {
  return readFileSync(resolve(ROOT, relativePath), 'utf8')
}

/** Every single-file component in both component layers. */
function components(): string[] {
  const roots = ['packages/design/src/components', 'apps/web/src']
  const found: string[] = []
  const walk = (dir: string): void => {
    for (const entry of readdirSync(resolve(ROOT, dir), { withFileTypes: true })) {
      const path = `${dir}/${entry.name}`
      if (entry.isDirectory()) {
        if (entry.name === 'node_modules' || entry.name === 'dist') continue
        walk(path)
      } else if (entry.name.endsWith('.vue')) {
        found.push(path)
      }
    }
  }
  for (const root of roots) walk(root)
  return found
}

/** The `<style>` bodies of a component, so script text cannot match a rule. */
function styles(css: string): string {
  return css.replace(/<script[\s\S]*?<\/script>/g, '')
}

const L0_RAMP = /var\(\s*--(?:gray|blue|green|orange|red|purple|yellow|pink)(?:-\w+)?\s*\)/

/**
 * A colour that is neither a hairline, a shadow, nor a preview of another theme.
 *
 * The preview card is the documented exception: it draws the two themes it is
 * comparing, so it cannot borrow the tokens of the one it is currently in.
 */
const LITERAL_PAINT = /(?<![\w-])(?:color|background|background-color|border-color|outline-color)\s*:\s*(?:#|rgb)/

const PREVIEW_EXCEPTION = 'packages/design/src/components/ThemePreviewCard.vue'

const SPACING_PROPERTY =
  /(?:^|;)\s*(?:gap|margin|padding|inset|top|right|bottom|left)(?:-[a-z]+)?\s*:\s*([^;]+)/g

/** A bare length in a spacing property, excluding the hairlines and zeroes. */
function bareSpacing(css: string): string[] {
  const found: string[] = []
  for (const match of css.matchAll(SPACING_PROPERTY)) {
    const value = match[1] ?? ''
    for (const length of value.matchAll(/(?<![\w-])(\d+(?:\.\d+)?)px/g)) {
      const size = Number(length[1])
      // A 1px or 2px offset is a drawn edge rather than a spacing step, and a
      // 0 is not a step at all.
      if (size > 2) found.push(match[0].trim())
    }
  }
  return found
}

describe('component layer migration', () => {
  it('paints with intents and app roles rather than raw ramp steps', () => {
    const failures: string[] = []
    for (const path of components()) {
      const css = styles(source(path))
      if (L0_RAMP.test(css)) failures.push(path)
    }
    expect(failures).toEqual([])
  })

  it('keeps literal paint inside the theme preview that has to draw it', () => {
    const failures: string[] = []
    for (const path of components()) {
      if (path === PREVIEW_EXCEPTION) continue
      const css = styles(source(path))
      if (LITERAL_PAINT.test(css)) failures.push(path)
    }
    expect(failures).toEqual([])
  })

  it('spaces with the scale rather than hand-picked lengths', () => {
    const failures: string[] = []
    for (const path of components()) {
      const css = styles(source(path))
      if (path === PREVIEW_EXCEPTION) continue
      for (const value of bareSpacing(css)) failures.push(`${path}: ${value}`)
    }
    expect(failures).toEqual([])
  })
})
