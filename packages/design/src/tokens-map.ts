/**
 * A typed index of the token names the design system declares, task U0-11.
 *
 * The stylesheet is the source of truth; this map exists so tests and tooling
 * can name a group of tokens once instead of each spelling the names it happens
 * to care about. The map is checked against the stylesheet rather than trusted,
 * because a name here that the stylesheet stopped declaring is a test that
 * silently covers nothing.
 *
 * The groups follow the token architecture's layers: the neutral ramp, the
 * accent ramps, the alpha scale, and then the semantic layers built on top.
 */

const NEUTRAL_STEPS = [
  '0', '25', '50', '75', '100', '150', '200', '250', '300', '350', '400', '450', '500',
  '550', '600', '650', '700', '750', '800', '850', '900', '925', '950', '975', '1000',
] as const

const ALPHA_STEPS = [
  '0', '01', '02', '04', '05', '06', '08', '10', '12', '15', '16', '20', '25', '30',
  '35', '40', '50', '60', '70',
] as const

/** The ramps the stylesheet declares, with the alpha variants each hue carries. */
const RAMP_STEPS: Record<string, readonly string[]> = {
  blue: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
  green: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
  orange: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
  red: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
  purple: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
  pink: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
  yellow: ['25', '50', '75', '100', '200', '300', '400', '500', '600', '700', '800', '900', '1000'],
}

const ALPHA_RAMP_STEPS = ['25', '50', '75', '100', '200', '300'] as const

const HEADING_SCALE_STEPS = ['xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl'] as const
const TEXT_SCALE_STEPS = ['3xs', '2xs', 'xs', 'sm', 'md', 'lg'] as const
const SMALL_CAPS_STEPS = ['md', 'lg'] as const
const TYPE_SCALE_PROPERTIES = ['size', 'line-height', 'tracking', 'weight'] as const

const ELEVATION_STEPS = ['100', '200', '300', '400'] as const
const SHADOW_STEPS = ['sm', 'md', 'lg'] as const

const INTENTS = ['info', 'success', 'warning', 'caution', 'danger', 'discovery'] as const
const TEXT_ROLES = [
  'emphasis',
  'default',
  'subtle',
  'inverse',
  'disabled',
  'primary',
  'secondary',
  'tertiary',
] as const
const SURFACE_ROLES = ['base', 'secondary', 'tertiary', 'elevated'] as const
const BORDER_ROLES = [
  'subtle',
  'default',
  'emphasis',
  'strong',
  'focus',
  'control',
  'control-hover',
  'base',
] as const
const RADIUS_STEPS = ['2xs', 'xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', 'full'] as const

function names(prefix: string, steps: readonly string[]): readonly string[] {
  return steps.map((step) => `${prefix}-${step}`)
}

/** Every property of one step of a type scale, e.g. `--font-heading-lg-size`. */
function typeScale(scale: string, steps: readonly string[]): readonly string[] {
  return steps.flatMap((step) =>
    TYPE_SCALE_PROPERTIES.map((property) => `--font-${scale}-${step}-${property}`),
  )
}

export const TOKEN_GROUPS = {
  neutral: [...names('--gray-fixed', NEUTRAL_STEPS), ...names('--gray', NEUTRAL_STEPS)],
  accents: [
    ...Object.entries(RAMP_STEPS).flatMap(([hue, steps]) => names(`--${hue}`, steps)),
    ...Object.entries(RAMP_STEPS).flatMap(([hue]) =>
      ALPHA_RAMP_STEPS.map((step) => `--${hue}-a${step}`),
    ),
  ],
  alpha: [...names('--alpha', ALPHA_STEPS), '--alpha-base'],
  intents: INTENTS.flatMap((intent) => [
    `--color-intent-${intent}-text`,
    `--color-intent-${intent}-surface`,
    `--color-intent-${intent}-border`,
    `--color-intent-${intent}-solid`,
  ]),
  text: names('--color-text', TEXT_ROLES),
  surfaces: names('--color-surface', SURFACE_ROLES),
  borders: names('--color-border', BORDER_ROLES),
  radii: [
    ...names('--radius', RADIUS_STEPS),
    ...names('--radius', RADIUS_STEPS).map((name) => `${name}-base`).filter(
      (name) => name !== '--radius-full-base',
    ),
    '--corner-radius-scale',
  ],
  motion: [
    '--ease-out',
    '--ease-in-out',
    '--ease-enter',
    '--ease-enter-snappy',
    '--cubic-enter',
    '--cubic-exit',
    '--cubic-exit-snappy',
    '--cubic-move',
    '--transition-fast',
    '--transition-normal',
    '--transition-slow',
  ],
  density: ['--density-row-height', '--density-gap', '--hit-target-min'],
  typography: [
    ...typeScale('heading', HEADING_SCALE_STEPS),
    ...typeScale('text', TEXT_SCALE_STEPS),
    ...typeScale('small-caps', SMALL_CAPS_STEPS),
    '--font-body',
    '--font-mono',
    '--font-display',
    '--font-serif',
    '--font-ui-sans',
    '--font-ui-serif',
    '--leading-tight',
    '--leading-normal',
    '--leading-relaxed',
    '--tracking-tight',
    '--tracking-normal',
    '--tracking-wide',
    '--weight-light',
    '--weight-normal',
    '--weight-medium',
    '--weight-semibold',
    '--weight-bold',
  ],
  elevation: [
    ...ELEVATION_STEPS.flatMap((step) => [
      `--elevation-${step}-geo`,
      `--shadow-${step}`,
      `--shadow-alpha-${step}`,
    ]),
    ...names('--shadow', SHADOW_STEPS),
    '--shadow-color',
    '--elevation-stroke',
    '--elevation-rail',
    '--elevation-inspector',
    '--elevation-panel',
    '--elevation-composer',
    '--elevation-composer-dark',
    '--border-width-hairline',
  ],
  shell: [
    '--rail-width',
    '--rail-min-width',
    '--rail-preferred-width',
    '--rail-max-width',
    '--rail-surface',
    '--rail-blur',
    '--transcript-min-width',
    '--inspector-width',
    '--inspector-width-dynamic',
    '--inspector-min-width',
    '--inspector-max-width',
    '--tabstrip-height',
    '--bottom-panel-height',
    '--bottom-panel-min-height',
    '--bottom-panel-max-height',
    '--composer-max-width',
    '--composer-radius',
    '--window-chrome-height',
    '--titlebar-hit-target',
  ],
} as const

export type TokenGroup = keyof typeof TOKEN_GROUPS

/** Every name the map holds, in group order and without repeats. */
export function tokenNames(): readonly string[] {
  const seen = new Set<string>()
  for (const group of Object.values(TOKEN_GROUPS)) {
    for (const name of group) seen.add(name)
  }
  return [...seen]
}
