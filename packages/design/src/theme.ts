/**
 * The two appearance axes, and the DOM writes that apply them.
 *
 * Kept free of Vue and of `localStorage` so it can be called from anywhere: an
 * inline `<head>` script that applies the stored theme before first paint (to
 * avoid the white flash) has no framework and no module graph available to it.
 */

export const THEME_MODES = ['light', 'dark'] as const
export type ThemeMode = (typeof THEME_MODES)[number]

export const COLOR_SCHEMES = ['codex', 'violet', 'teal', 'rose'] as const
export type ColorScheme = (typeof COLOR_SCHEMES)[number]

/** Dark, because this tool sits next to a terminal. */
export const DEFAULT_THEME: ThemeMode = 'dark'

/**
 * Orchester's own hue.
 *
 * The mark, the desktop icon and the send button are all the same pink, so the
 * default accent is the colour of the product rather than of the design it
 * learned its anatomy from.
 */
export const DEFAULT_COLOR_SCHEME: ColorScheme = 'rose'

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

/* ══ Intensity ══════════════════════════════════════════════════════════════
   How loudly the accent is allowed to speak.

   `vivid` spends the accent on the primary action. It is the default because
   Orchester's own surfaces are built that way: a governed run is consequential
   and the control that starts it should be the most findable thing on screen.
   `calm` keeps the accent for focus, links and active markers and renders the
   primary action as an inverted neutral, which is the restraint the surface
   this design learned from applies. Two words, one axis: a third would mean
   nobody could tell them apart, and eight would mean nobody would try. */

export const INTENSITIES = ['calm', 'vivid'] as const
export type Intensity = (typeof INTENSITIES)[number]

export const DEFAULT_INTENSITY: Intensity = 'vivid'
export const INTENSITY_ATTRIBUTE = 'data-intensity'
export const INTENSITY_STORAGE_KEY = 'orchester:intensity'

export function isIntensity(value: unknown): value is Intensity {
  return typeof value === 'string' && (INTENSITIES as readonly string[]).includes(value)
}

/* ══ Reduced motion ════════════════════════════════════════════════════════
   Three states, not two. `null` means "do what the operating system says",
   which is not the same as `'false'`: a user on a reduced-motion desktop who
   still wants our transitions has to be able to say so, and that requires an
   explicit value that can defeat the media query. */

export const REDUCED_MOTION_VALUES = ['true', 'false'] as const
export type ReducedMotionValue = (typeof REDUCED_MOTION_VALUES)[number]
/** `null` defers to `prefers-reduced-motion`. */
export type ReducedMotionPreference = ReducedMotionValue | null

export const REDUCED_MOTION_ATTRIBUTE = 'data-reduced-motion'
export const REDUCED_MOTION_STORAGE_KEY = 'orchester:reduced-motion'

export function isReducedMotionPreference(value: unknown): value is ReducedMotionPreference {
  return value === null || (REDUCED_MOTION_VALUES as readonly unknown[]).includes(value)
}

/* ══ Surface ════════════════════════════════════════════════════════════════
   Which face is rendering. This is a property of the build, not a taste, so it
   is announced by the host (the desktop shell sets it before the bundle loads)
   and is deliberately never persisted. */

export const SURFACES = ['web', 'site', 'desktop'] as const
export type Surface = (typeof SURFACES)[number]

export const DEFAULT_SURFACE: Surface = 'web'
export const SURFACE_ATTRIBUTE = 'data-orchester-surface'

export function isSurface(value: unknown): value is Surface {
  return typeof value === 'string' && (SURFACES as readonly string[]).includes(value)
}

/* ═ Platform ═══════════════════════════════════════════════════════════════
   Chrome differs per platform — traffic lights on one side, caption buttons on
   the other — and doing that with a user-agent test inside every component
   would spread the same string match across the tree. It is resolved once,
   here, and published as a root attribute. */

export const PLATFORMS = ['macos', 'windows', 'linux'] as const
export type Platform = (typeof PLATFORMS)[number]

export const OS_ATTRIBUTE = 'data-orchester-os'

/**
 * Which platform a user-agent string describes.
 *
 * Falls back to Linux rather than guessing: an unrecognised string is far more
 * likely to be a stripped-down webview than a phone, and of the three mistakes
 * available this is the one that lays out a window sensibly everywhere.
 */
export function detectPlatform(userAgent: string): Platform {
  const value = userAgent.toLowerCase()
  if (value.includes('mac os') || value.includes('macintosh')) return 'macos'
  if (value.includes('windows')) return 'windows'
  return 'linux'
}

export function readSystemPlatform(): Platform {
  if (typeof navigator === 'undefined') return 'linux'
  const agent = typeof navigator.userAgent === 'string' ? navigator.userAgent : ''
  return detectPlatform(agent)
}

export function applyIntensityToDocument(intensity: Intensity): void {
  if (!hasDocument()) return
  document.documentElement.setAttribute(INTENSITY_ATTRIBUTE, intensity)
}

export function applyReducedMotionToDocument(preference: ReducedMotionPreference): void {
  if (!hasDocument()) return
  const root = document.documentElement
  // `null` removes the attribute rather than writing a third value, so the
  // stylesheet's `:not([data-reduced-motion='false'])` guard can tell "no
  // opinion" apart from "explicitly on".
  if (preference === null) root.removeAttribute(REDUCED_MOTION_ATTRIBUTE)
  else root.setAttribute(REDUCED_MOTION_ATTRIBUTE, preference)
}

export function applySurfaceToDocument(surface: Surface): void {
  if (!hasDocument()) return
  document.documentElement.setAttribute(SURFACE_ATTRIBUTE, surface)
}

export function applyPlatformToDocument(platform: Platform): void {
  if (!hasDocument()) return
  document.documentElement.setAttribute(OS_ATTRIBUTE, platform)
}

export function readDocumentIntensity(): Intensity | null {
  if (!hasDocument()) return null
  const current = document.documentElement.getAttribute(INTENSITY_ATTRIBUTE)
  return isIntensity(current) ? current : null
}

export function readDocumentReducedMotion(): ReducedMotionPreference {
  if (!hasDocument()) return null
  const current = document.documentElement.getAttribute(REDUCED_MOTION_ATTRIBUTE)
  return current === 'true' || current === 'false' ? current : null
}

export function readDocumentSurface(): Surface | null {
  if (!hasDocument()) return null
  const current = document.documentElement.getAttribute(SURFACE_ATTRIBUTE)
  return isSurface(current) ? current : null
}

export function readDocumentPlatform(): Platform | null {
  if (!hasDocument()) return null
  const current = document.documentElement.getAttribute(OS_ATTRIBUTE)
  return (PLATFORMS as readonly string[]).includes(current ?? '') ? (current as Platform) : null
}
