/**
 * The hex values the theme's background and foreground roles resolve to.
 *
 * The appearance table prints the value beside each swatch, as the reference
 * does. Reading it back out of the DOM would be the honest source, but a
 * component cannot resolve a chained custom property - the shell's stylesheet
 * is not loaded in every surface that mounts this view, and jsdom does not
 * substitute `var()` at all - so the pair is mirrored here and checked against
 * `tokens.css` by a test rather than trusted.
 */

import type { ThemeMode } from '@orchester/design'

export interface ThemeColours {
  background: string
  foreground: string
}

/**
 * One entry per theme, spelling the end of the chain each role walks:
 * `--color-bg-base` -> `--color-surface-tertiary` -> a neutral step, and
 * `--color-text-primary` -> `--color-text-emphasis` -> a neutral step.
 */
export const THEME_COLOURS: Record<ThemeMode, ThemeColours> = {
  dark: { background: '#131313', foreground: '#FFFFFF' },
  light: { background: '#F3F3F3', foreground: '#0D0D0D' },
}

/** The pair for the theme in force, normalised to upper case for display. */
export function themeColours(theme: ThemeMode): ThemeColours {
  return THEME_COLOURS[theme]
}
