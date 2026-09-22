/**
 * The hex values each theme's roles resolve to.
 *
 * The appearance cards print the value beside each swatch, as the reference
 * does. Reading it back out of the DOM would be the honest source, but a
 * component cannot resolve a chained custom property - the shell's stylesheet
 * is not loaded in every surface that mounts this view, and jsdom does not
 * substitute `var()` at all - so the values are mirrored here and checked
 * against `tokens.css` by a test rather than trusted.
 *
 * The accent is mirrored per theme *and* scheme, because the same scheme is two
 * colours: the light variants are darkened until accent-on-base clears AA for
 * body text, which is why one swatch could not honestly stand for both.
 */

import type { ColorScheme, ThemeMode } from '@orchester/design'

export interface ThemeColours {
  background: string
  foreground: string
  /** The hue this theme paints each scheme's accent in. */
  accent: Record<ColorScheme, string>
  /** The text colour to set on top of that accent, per scheme. */
  accentContrast: Record<ColorScheme, string>
}

/**
 * One entry per theme, spelling the end of the chain each role walks:
 * `--color-bg-base` -> `--color-surface-tertiary` -> a neutral step,
 * `--color-text-primary` -> `--color-text-emphasis` -> a neutral step, and
 * `--color-accent` -> the scheme's own ramp step.
 */
export const THEME_COLOURS: Record<ThemeMode, ThemeColours> = {
  dark: {
    background: '#131313',
    foreground: '#FFFFFF',
    accent: {
      codex: '#339CFF',
      violet: '#A58BF0',
      teal: '#4FBFAD',
      rose: '#F472B6',
    },
    accentContrast: {
      codex: '#0D1524',
      violet: '#130F22',
      teal: '#08201D',
      rose: '#1A0512',
    },
  },
  light: {
    background: '#F3F3F3',
    foreground: '#0D0D0D',
    accent: {
      codex: '#0169CC',
      violet: '#6244C4',
      teal: '#1C7568',
      rose: '#C0267E',
    },
    accentContrast: {
      codex: '#FFFFFF',
      violet: '#FFFFFF',
      teal: '#FFFFFF',
      rose: '#FFFFFF',
    },
  },
}

/** The values for the theme in force, normalised to upper case for display. */
export function themeColours(theme: ThemeMode): ThemeColours {
  return THEME_COLOURS[theme]
}

/** The accent one theme paints one scheme in. */
export function themeAccent(theme: ThemeMode, scheme: ColorScheme): string {
  return THEME_COLOURS[theme].accent[scheme]
}

/** The text colour that rides on top of that accent. */
export function themeAccentContrast(theme: ThemeMode, scheme: ColorScheme): string {
  return THEME_COLOURS[theme].accentContrast[scheme]
}

/**
 * One theme as the text "Copy theme" puts on the clipboard.
 *
 * The reference offers to copy the theme rather than to screenshot it, and the
 * shape that is worth copying is the resolved one: the values the reader can
 * see on the card, named, so the paste is readable without this stylesheet.
 */
export function themeClipboardPayload(theme: ThemeMode, scheme: ColorScheme): string {
  const colours = THEME_COLOURS[theme]
  return JSON.stringify(
    {
      theme,
      colorScheme: scheme,
      accent: themeAccent(theme, scheme),
      background: colours.background,
      foreground: colours.foreground,
    },
    null,
    2,
  )
}