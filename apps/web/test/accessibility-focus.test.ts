import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * The focus half of §7.
 *
 * "Never remove without replacing" is the one rule in the accessibility
 * contract that is easy to break by accident, because `outline: none` is what
 * a stylesheet reaches for whenever the default ring looks wrong. A rule that
 * removes the ring and replaces nothing leaves a keyboard user with a state
 * they cannot see, so every removal in the product has to be justified by a
 * replacement in the same declaration block.
 */

function source(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), 'src', relativePath), 'utf8')
}

interface Block {
  selector: string
  body: string
}

/** Every top-level declaration block in a stylesheet, with its selector. */
function blocks(css: string): Block[] {
  const found: Block[] = []
  const pattern = /([^{}]+)\{([^{}]*)\}/g
  for (const match of css.matchAll(pattern)) {
    found.push({ selector: match[1]!.trim(), body: match[2]! })
  }
  return found
}

const STYLESHEETS = [
  'views/SettingsView.vue',
  'components/layout/WorkspaceHeader.vue',
  'components/layout/WindowChrome.vue',
  'components/run/RunComposer.vue',
  'components/run/PlanStrip.vue',
  'components/run/ToolCallCard.vue',
  'components/run/ReasoningDisclosure.vue',
  'components/run/CommandPalette.vue',
  'components/changes/ChangeInspector.vue',
  'components/sessions/SessionListItem.vue',
  'components/sessions/SessionTranscript.vue',
  'features/agent-presence/components/AgentFleetRow.vue',
]

describe('focus visibility contract', () => {
  /**
   * The wrapper an element sits in, when a `:focus-within` rule on that
   * wrapper paints the boundary on the element's behalf. A borderless field
   * inside a bordered box is the reason this exists: the box is what changes,
   * and the field's own ring would double it.
   */
  function hasWrapperReplacement(path: string, selector: string): boolean {
    // The wrapper is the class the element sits inside: for
    // `.field input:focus` it is `.field`, not `input`.
    const parts = selector.split(/\s+/)
    if (parts.length < 2) return false
    const wrapper = parts[parts.length - 2]!.trim()
    if (!wrapper.startsWith('.')) return false
    return blocks(source(path)).some(
      (block) =>
        block.selector.startsWith(wrapper) &&
        block.selector.includes(':focus-within') &&
        (/outline(-offset)?:\s*(?!none|0)/.test(block.body) || /box-shadow:/.test(block.body)),
    )
  }

  it('never removes a focus outline without painting a replacement', () => {
    const failures: string[] = []
    for (const path of STYLESHEETS) {
      for (const block of blocks(source(path))) {
        if (!/outline:\s*(none|0)\b/.test(block.body)) continue
        // The block may replace the ring on the element itself, and a wrapper
        // may replace it on the element's behalf through `:focus-within`.
        const replacesInline = /outline(-offset)?:\s*(?!none|0)/.test(
          block.body.replace(/outline:\s*(none|0)\b/, ''),
        )
        const replacesWithShadow = /box-shadow:/.test(block.body)
        const isWrapper = /:focus-within/.test(block.selector)
        const wrapperReplaces = hasWrapperReplacement(path, block.selector)
        if (!replacesInline && !replacesWithShadow && !isWrapper && !wrapperReplaces) {
          failures.push(`${path}: ${block.selector}`)
        }
      }
    }
    expect(failures).toEqual([])
  })

  it('gives the focus ring a dedicated token rather than falling back to a hex', () => {
    // A hard-coded fallback means the ring silently stops following the theme
    // and the brand hue, which is how the menu ended up painting orange in a
    // pink-and-blue product.
    const failures: string[] = []
    for (const path of STYLESHEETS) {
      for (const block of blocks(source(path))) {
        for (const match of block.body.matchAll(/outline:\s*[^;]*var\((--[a-z0-9-]+),\s*#[0-9a-fA-F]/g)) {
          failures.push(`${path}: ${block.selector} falls back from ${match[1]}`)
        }
      }
    }
    expect(failures).toEqual([])
  })

  it('draws the focus boundary for a borderless field on its wrapper', () => {
    // The settings search is the one input that removes the shared ring. Its
    // replacement is a `:focus-within` boundary on the wrapper, which this
    // pins so the removal cannot outlive the replacement.
    const view = source('views/SettingsView.vue')

    expect(view).toMatch(/\.settings-view__search:focus-within\s*\{[^}]*border-color:[^}]*--color-border-focus/)
  })

  it('routes the settings search boundary through the control token', () => {
    // The search box is a control, so its resting boundary is the one held to
    // the 3:1 non-text bar, not the separator role.
    const view = source('views/SettingsView.vue')

    expect(view).toMatch(/\.settings-view__search\s*\{[^}]*border:\s*1px solid var\(--color-border-control\)/)
  })
})
