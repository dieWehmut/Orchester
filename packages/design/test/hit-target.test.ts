import { readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * The hit-target half of the accessibility contract in section 7.
 *
 * "Minimum hit target 32x32 px, 40x40 in compact density" is the clause a
 * pointer user depends on: a control whose box is smaller than the floor is one
 * they have to aim at rather than click. The floor is expressed once as
 * `--hit-target-min`, so the density axis can raise it without every component
 * knowing which density is active.
 *
 * Two shapes count as meeting it. A control can grow its own box to the floor,
 * which is what a button does. A control whose drawing has to stay small - a
 * 16 px swatch, a 20 px switch track - grows the box around the drawing
 * instead, so the target is the floor even though the paint is not.
 */

const tokens = source('src/tokens.css')

function source(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), relativePath), 'utf8')
}

/** Every top-level declaration block in a stylesheet, with its selector. */
function blocks(css: string): { selector: string; body: string }[] {
  const found: { selector: string; body: string }[] = []
  // Comments sit above the rule they explain, and a single-file component's
  // opening `<style>` tag sits directly above its first rule, so both are
  // stripped or the first selector reads as `<style scoped> .icon-button`.
  const stripped = css
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/<style[^>]*>/g, '')
  for (const match of stripped.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    found.push({ selector: match[1]!.trim(), body: match[2]! })
  }
  return found
}

/** The declarations of one rule, from the component's own stylesheet. */
function declarations(path: string, selector: string): string {
  const found = blocks(stylesheet(source(path))).filter((block) => block.selector === selector)
  if (found.length === 0) throw new Error(`${path} declares no rule for ${selector}`)
  return found.map((block) => block.body).join('\n')
}

/** The `<style>` body of a single-file component, without its script. */
function stylesheet(css: string): string {
  const start = css.search(/<style[^>]*>/)
  return start < 0 ? css : css.slice(start)
}

const COMPONENT_DIR = 'src/components'
const COMPONENTS = readdirSync(resolve(process.cwd(), COMPONENT_DIR))
  .filter((name) => name.endsWith('.vue'))
  .map((name) => `${COMPONENT_DIR}/${name}`)

/** The classes that are a target on their own, audited control by control. */
const FLOORED_CONTROLS: readonly [path: string, selector: string][] = [
  ['src/components/IconButton.vue', '.icon-button'],
  ['src/components/AppButton.vue', '.app-button'],
  ['src/components/AppDialog.vue', '.app-dialog__close'],
  ['src/components/AppDrawer.vue', '.app-drawer__close'],
  ['src/components/InlineAlert.vue', '.inline-alert__dismiss'],
  ['src/components/ToastRegion.vue', '.toast-region__dismiss'],
  ['src/components/AppCheckbox.vue', '.app-checkbox'],
  ['src/components/AppSwitch.vue', '.app-switch'],
  ['src/components/AppMenu.vue', '.app-menu__trigger'],
  ['src/components/AppMenu.vue', '.app-menu__item'],
  ['src/components/AppTabs.vue', '.app-tabs__tab'],
  ['src/components/AppSegmentedControl.vue', '.app-segmented-control__option'],
  ['src/components/EmptyState.vue', '.empty-state__button'],
]

describe('hit target contract', () => {
  it('names the floor and raises it under compact density', () => {
    expect(declarations('src/tokens.css', ':root')).toMatch(/--hit-target-min:\s*32px/)
    expect(declarations('src/tokens.css', ":root[data-density='compact']")).toMatch(
      /--hit-target-min:\s*40px/,
    )
  })

  it('keeps the compact floor above the comfortable one', () => {
    // Compact tightens the visuals, so the floor is what has to grow back: the
    // clause reads "40x40 in compact", never a smaller number than 32.
    const split = tokens.indexOf(":root[data-density='compact']")
    const floorOf = (css: string): number => Number(/--hit-target-min:\s*(\d+)px/.exec(css)?.[1] ?? 0)
    expect(floorOf(tokens.slice(split))).toBeGreaterThan(floorOf(tokens.slice(0, split)))
  })

  it('floors every standalone control on the hit-target token', () => {
    const failures: string[] = []
    for (const [path, selector] of FLOORED_CONTROLS) {
      const body = declarations(path, selector)
      // Either the box itself is the floor, or a larger control-height is held
      // above it with `max()` so compact can still raise the target.
      const floored =
        /min-(?:block|inline)-size:\s*var\(--hit-target-min/.test(body) ||
        /min-(?:block|inline)-size:\s*max\([^)]*--hit-target-min/.test(body)
      if (!floored) failures.push(`${path}: ${selector}`)
    }
    expect(failures).toEqual([])
  })

  it('gives every control wider than it is tall a floor in the block axis', () => {
    // A field is as wide as its column, so the axis it owes is the block one.
    for (const [path, selector] of [
      ['src/components/AppInput.vue', '.app-input'],
      ['src/components/AppSelect.vue', '.app-select'],
      ['src/components/AppTextarea.vue', '.app-textarea'],
    ] as const) {
      expect(declarations(path, selector), `${path}: ${selector}`).toMatch(
        /min-height:\s*(?:max\()?var\(--(?:hit-target-min|control-height)/,
      )
    }
  })

  it('keeps a control that paints smaller than the floor on the floor box', () => {
    // The swatch draws a 16 px dot on purpose. It satisfies the clause by
    // sitting in a floor-sized button rather than by painting a big dot.
    const swatch = declarations('src/components/ColorSchemePicker.vue', '.scheme-picker__swatch')
    expect(swatch).toMatch(/inline-size:\s*var\(--hit-target-min/)
    expect(swatch).toMatch(/block-size:\s*var\(--hit-target-min/)
  })

  it('never declares a sub-floor fixed box for an interactive class without the floor', () => {
    // The shape this clause exists to catch: a hand-picked `22px` on a control
    // whose box is what the pointer aims at. A file may keep a small drawing,
    // but then it has to say so by referencing the floor itself.
    const failures: string[] = []
    for (const path of COMPONENTS) {
      const css = source(path)
      if (css.includes('--hit-target-min')) continue
      for (const block of blocks(css)) {
        if (block.selector.includes('::')) continue
        if (!/__(?:close|dismiss|toggle|trigger|option|tab|control|button)\b|\bbutton\b/.test(block.selector)) continue
        for (const match of block.body.matchAll(/(?:^|;)\s*(width|height|inline-size|block-size)\s*:\s*(\d+)px/g)) {
          const size = Number(match[2])
          if (size > 0 && size < 32) failures.push(`${path}: ${block.selector} -> ${match[0].trim()}`)
        }
      }
    }
    expect(failures).toEqual([])
  })
})