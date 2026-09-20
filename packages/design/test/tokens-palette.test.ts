import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const tokens = readFileSync(resolve(process.cwd(), 'src/tokens.css'), 'utf8')

/** The declarations made by one selector block, so theme blocks can be compared. */
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
const root = block(':root')

const NEUTRAL_STEPS = [
  '0', '25', '50', '75', '100', '150', '200', '250', '300', '350', '400', '450',
  '500', '550', '600', '650', '700', '750', '800', '850', '900', '925', '950',
  '975', '1000',
] as const

const HUES = ['blue', 'green', 'orange', 'red', 'purple', 'yellow'] as const
const HUE_STEPS = ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'] as const

const INTENTS = ['info', 'success', 'warning', 'caution', 'danger', 'discovery'] as const
const TREATMENTS = ['text', 'surface', 'border', 'solid'] as const

const ALPHA_STEPS = ['0', '01', '02', '04', '05', '06', '08', '10', '12', '15', '16', '20', '25', '30', '35', '40', '50', '60', '70'] as const

describe('neutral ramp', () => {
  it('declares every fixed step once, outside the theme blocks', () => {
    for (const step of NEUTRAL_STEPS) {
      expect(root).toContain(`--gray-fixed-${step}:`)
    }
  })

  it('flips every theme-aware alias in both themes', () => {
    for (const step of NEUTRAL_STEPS) {
      expect(dark, `dark --gray-${step}`).toContain(`--gray-${step}:`)
      expect(light, `light --gray-${step}`).toContain(`--gray-${step}:`)
    }
  })

  it('anchors the dark surface at near-black and the light surface at white', () => {
    expect(dark).toMatch(/--gray-0:\s*#0d0d0d/)
    expect(light).toMatch(/--gray-0:\s*#fff/)
    expect(dark).toMatch(/--gray-1000:\s*#fff/)
    expect(light).toMatch(/--gray-1000:\s*#0d0d0d/)
  })
})

describe('accent ramps', () => {
  it('declares every step of every hue', () => {
    for (const hue of HUES) {
      for (const step of HUE_STEPS) {
        expect(root, `--${hue}-${step}`).toContain(`--${hue}-${step}:`)
      }
    }
  })

  it('declares the alpha variants of every hue', () => {
    for (const hue of HUES) {
      for (const a of ['25', '50', '75', '100', '200', '300']) {
        expect(root, `--${hue}-a${a}`).toContain(`--${hue}-a${a}:`)
      }
    }
  })

  it('points the codex scheme at the blue ramp', () => {
    expect(tokens).toMatch(
      /\[data-theme='dark'\]\[data-color-scheme='codex'\][^}]*--color-accent:\s*var\(--blue-300\)/s,
    )
    expect(tokens).toMatch(
      /\[data-theme='light'\]\[data-color-scheme='codex'\][^}]*--color-accent:\s*var\(--blue-500\)/s,
    )
  })
})

describe('alpha scale', () => {
  it('declares every step in both themes', () => {
    for (const step of ALPHA_STEPS) {
      expect(dark, `dark --alpha-${step}`).toContain(`--alpha-${step}:`)
      expect(light, `light --alpha-${step}`).toContain(`--alpha-${step}:`)
    }
  })

  it('derives every step from the theme-dependent alpha base', () => {
    for (const step of ALPHA_STEPS) {
      expect(dark).toMatch(new RegExp(`--alpha-${step}:\\s*color-mix\\([^;]*var\\(--alpha-base\\)`))
    }
  })

  it('flips the base from dark ink to white', () => {
    expect(dark).toMatch(/--alpha-base:\s*#fff/)
    expect(light).toMatch(/--alpha-base:\s*#0d0d0d/)
  })
})

describe('intent matrix', () => {
  it('gives every intent all four treatments in both themes', () => {
    for (const intent of INTENTS) {
      for (const treatment of TREATMENTS) {
        const token = `--color-intent-${intent}-${treatment}`
        expect(dark, `dark ${token}`).toContain(token)
        expect(light, `light ${token}`).toContain(token)
      }
    }
  })

  it('declares the text, surface and border roles in both themes', () => {
    const roles = [
      '--color-text-emphasis', '--color-text-default', '--color-text-subtle',
      '--color-text-disabled',
      '--color-surface-base', '--color-surface-secondary', '--color-surface-tertiary',
      '--color-surface-elevated',
      '--color-border-subtle', '--color-border-default', '--color-border-emphasis',
      '--color-border-focus',
    ]
    for (const role of roles) {
      expect(dark, `dark ${role}`).toContain(role)
      expect(light, `light ${role}`).toContain(role)
    }
  })

  it('keeps the new role names clear of the legacy token names they will replace', () => {
    // The migration in wave U8 re-points the legacy names at these. Until then
    // both sets must coexist, so a collision here would be a silent override.
    const legacy = [
      '--color-text-primary:', '--color-text-secondary:', '--color-text-tertiary:',
      '--color-border-base:', '--color-border-strong:',
    ]
    for (const name of legacy) {
      expect(tokens).toContain(name)
    }
    for (const name of ['--color-text-default:', '--color-border-default:']) {
      expect(tokens).toContain(name)
    }
  })

  it('keeps the focus border on the brand hue in both themes', () => {
    expect(dark).toMatch(/--color-border-focus:\s*var\(--pink-300\)/)
    expect(light).toMatch(/--color-border-focus:\s*var\(--pink-500\)/)
  })
})

describe('type scales', () => {
  const HEADING = ['xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl'] as const
  const TEXT = ['3xs', '2xs', 'xs', 'sm', 'md', 'lg'] as const

  it('binds size, line-height, tracking and weight for every heading step', () => {
    for (const step of HEADING) {
      for (const part of ['size', 'line-height', 'tracking', 'weight']) {
        expect(root, `--font-heading-${step}-${part}`).toContain(`--font-heading-${step}-${part}:`)
      }
    }
  })

  it('binds size, line-height, tracking and weight for every text step', () => {
    for (const step of TEXT) {
      for (const part of ['size', 'line-height', 'tracking', 'weight']) {
        expect(root, `--font-text-${step}-${part}`).toContain(`--font-text-${step}-${part}:`)
      }
    }
  })

  it('orders every step of both scales by size, so a scale is a scale', () => {
    // The plan asks for monotonicity: `lg` must be larger than `md`. A scale
    // that is merely bound is not yet ordered, and an unordered scale renders
    // headings that jump around between levels.
    const size = (name: string): number => {
      const match = root.match(new RegExp(`--font-${name}-size:\\s*([0-9.]+)rem`))
      if (!match) throw new Error(`missing size for ${name}`)
      return Number(match[1])
    }
    for (const scale of ['heading', 'text'] as const) {
      const steps = scale === 'heading' ? HEADING : TEXT
      const sizes = steps.map((step) => size(`${scale}-${step}`))
      for (let i = 1; i < sizes.length; i += 1) {
        expect(sizes[i], `${scale}-${steps[i]} > ${scale}-${steps[i - 1]}`).toBeGreaterThan(
          sizes[i - 1]!,
        )
      }
    }
  })

  it('declares the tracking and weight primitives the scales reference', () => {
    for (const token of ['--tracking-tight', '--tracking-normal', '--tracking-wide']) {
      expect(root).toContain(token)
    }
    for (const weight of ['light', 'normal', 'medium', 'semibold', 'bold']) {
      expect(root).toContain(`--weight-${weight}:`)
    }
  })
})

describe('shape scale', () => {
  it('declares a base radius for every step', () => {
    for (const step of ['2xs', 'xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl']) {
      expect(root, `--radius-${step}-base`).toContain(`--radius-${step}-base:`)
    }
  })

  it('derives the new radius steps through the corner-radius multiplier', () => {
    for (const step of ['2xs', '2xl', '3xl', '4xl']) {
      expect(root).toMatch(
        new RegExp(`--radius-${step}:\\s*calc\\(var\\(--radius-${step}-base\\) \\* var\\(--corner-radius-scale\\)\\)`),
      )
    }
  })

  it('resolves the large radius the plan names', () => {
    // The plan states the resolved value, not just the derivation: 0.625rem at
    // a 1.25 multiplier is 0.78125rem. Asserting the derivation alone would
    // pass if either number drifted.
    const base = Number(root.match(/--radius-lg-base:\s*([0-9.]+)rem/)![1])
    const scale = Number(root.match(/--corner-radius-scale:\s*([0-9.]+)/)![1])
    expect(base * scale).toBeCloseTo(0.78125, 5)
  })

  it('gives Orchester the softer Codex corner', () => {
    expect(root).toMatch(/--corner-radius-scale:\s*1\.25/)
  })

  it('declares a true hairline width', () => {
    expect(root).toMatch(/--border-width-hairline:\s*\.5px/)
  })
})

describe('elevation', () => {
  it('separates geometry from colour so one shape can be reused at several strengths', () => {
    for (const step of ['100', '200', '300', '400']) {
      expect(root).toContain(`--elevation-${step}-geo:`)
      expect(root).toContain(`--shadow-${step}:`)
    }
  })

  it('offers the hairline as a stroke that does not affect layout', () => {
    expect(dark).toMatch(/--elevation-stroke:\s*0 0 0 var\(--border-width-hairline\)/)
    expect(light).toMatch(/--elevation-stroke:\s*0 0 0 var\(--border-width-hairline\)/)
  })

  it('names the shell surfaces', () => {
    for (const token of ['--elevation-rail', '--elevation-inspector', '--elevation-composer', '--elevation-panel']) {
      expect(dark, `dark ${token}`).toContain(token)
      expect(light, `light ${token}`).toContain(token)
    }
  })
})

describe('motion', () => {
  it('declares the enter, exit and move curves', () => {
    expect(root).toMatch(/--cubic-enter:\s*cubic-bezier\(\.19, 1, \.22, 1\)/)
    expect(root).toMatch(/--cubic-exit:\s*cubic-bezier\(\.8, 0, \.4, 1\)/)
    expect(root).toMatch(/--cubic-move:\s*cubic-bezier\(\.65, 0, \.35, 1\)/)
  })

  it('collapses every duration when reduced motion is requested or forced', () => {
    expect(tokens).toMatch(
      /:root\[data-reduced-motion='true'\][^}]*--transition-fast:\s*1ms/s,
    )
    expect(tokens).toMatch(
      /:root\[data-reduced-motion='true'\][^}]*--transition-normal:\s*1ms/s,
    )
    expect(tokens).toMatch(/@media \(prefers-reduced-motion: reduce\)/)
  })
})

describe('shell layout', () => {
  it('never lets the rail take the transcript below its minimum', () => {
    expect(root).toMatch(/--rail-width:\s*clamp\([^)]*100vw - 360px[^)]*\)/)
  })

  it('derives the inspector width from its own clamp', () => {
    expect(root).toMatch(/--inspector-width-dynamic:\s*clamp\(/)
  })

  it('declares the tab strip, bottom panel and composer metrics', () => {
    for (const token of [
      '--tabstrip-height', '--bottom-panel-height', '--bottom-panel-min-height',
      '--composer-max-width', '--composer-radius', '--thread-header-height',
    ]) {
      expect(root, token).toContain(`${token}:`)
    }
  })
})

describe('density', () => {
  it('declares the comfortable defaults', () => {
    expect(root).toMatch(/--density-row-height:\s*32px/)
    expect(root).toMatch(/--density-gap:\s*var\(--space-3\)/)
  })

  it('tightens under the compact axis', () => {
    expect(tokens).toMatch(/\[data-density='compact'\][^}]*--density-row-height:\s*26px/s)
  })
})