/**
 * The two appearance axes, and the DOM writes that apply them.
 *
 * Kept free of Vue and of `localStorage` so it can be called from anywhere: an
 * inline `<head>` script that applies the stored theme before first paint (to
 * avoid the white flash) has no framework and no module graph available to it.
 */

export const THEME_MODES = ['light', 'dark'] as const
export type ThemeMode = (typeof THEME_MODES)[number]

export const COLOR_SCHEMES = ['amber', 'violet', 'teal', 'rose'] as const
export type ColorScheme = (typeof COLOR_SCHEMES)[number]

/** Dark, because this tool sits next to a terminal. */
export const DEFAULT_THEME: ThemeMode = 'dark'

/** Brass, for an orchestra. */
export const DEFAULT_COLOR_SCHEME: ColorScheme = 'amber'

export const THEME_ATTRIBUTE = 'data-theme'
export const COLOR_SCHEME_ATTRIBUTE = 'data-color-scheme'

export const THEME_STORAGE_KEY = 'orchester:theme'
export const COLOR_SCHEME_STORAGE_KEY = 'orchester:color-scheme'

export function isThemeMode(value: unknown): value is ThemeMode {
  return typeof value === 'string' && (THEME_MODES as readonly string[]).includes(value)
}

export function isColorScheme(value: unknown): value is ColorScheme {
  return typeof value === 'string' && (COLOR_SCHEMES as readonly string[]).includes(value)
}

export interface ColorSchemeOption {
  id: ColorScheme
  /** An i18n key rather than a label: this package owns no copy. */
  labelKey: string
}

export const COLOR_SCHEME_OPTIONS: readonly ColorSchemeOption[] = COLOR_SCHEMES.map((id) => ({
  id,
  labelKey: `colorScheme.${id}`,
}))

/** Whether we are running somewhere with a DOM to write to. */
export function hasDocument(): boolean {
  return typeof document !== 'undefined'
}

export function applyThemeToDocument(theme: ThemeMode): void {
  if (!hasDocument()) return
  const root = document.documentElement
  root.setAttribute(THEME_ATTRIBUTE, theme)
  // Mirrored onto the CSS property so form controls, scrollbars and the
  // browser's own UI follow. The attribute alone only reaches our stylesheet.
  root.style.colorScheme = theme
}

export function applyColorSchemeToDocument(scheme: ColorScheme): void {
  if (!hasDocument()) return
  document.documentElement.setAttribute(COLOR_SCHEME_ATTRIBUTE, scheme)
}

export function readDocumentTheme(): ThemeMode | null {
  if (!hasDocument()) return null
  const current = document.documentElement.getAttribute(THEME_ATTRIBUTE)
  return isThemeMode(current) ? current : null
}

export function readDocumentColorScheme(): ColorScheme | null {
  if (!hasDocument()) return null
  const current = document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)
  return isColorScheme(current) ? current : null
}

/**
 * What the operating system asks for.
 *
 * Consulted only when the user has expressed no preference of their own. Once
 * they have clicked the toggle, their choice outranks the OS — someone who wants
 * a dark editor on a light desktop should not have it undone at midnight.
 */
export function readSystemTheme(): ThemeMode | null {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return null
  if (window.matchMedia('(prefers-color-scheme: light)').matches) return 'light'
  if (window.matchMedia('(prefers-color-scheme: dark)').matches) return 'dark'
  return null
}
