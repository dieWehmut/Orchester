import { computed, readonly, ref, type ComputedRef, type Ref } from 'vue'

import { clearStored, readStored, writeStored } from '../storage'
import {
  COLOR_SCHEME_STORAGE_KEY,
  COLOR_SCHEMES_STORAGE_KEY,
  CONTENT_FONT_STORAGE_KEY,
  CONTENT_FONT_WEIGHT_STORAGE_KEY,
  DEFAULT_COLOR_SCHEME,
  DEFAULT_CONTENT_FONT,
  DEFAULT_FONT_WEIGHT,
  DEFAULT_INTENSITY,
  DEFAULT_RAIL_APPEARANCE,
  DEFAULT_SURFACE,
  DEFAULT_THEME,
  DEFAULT_UI_FONT,
  INTENSITY_STORAGE_KEY,
  RAIL_APPEARANCE_STORAGE_KEY,
  REDUCED_MOTION_STORAGE_KEY,
  THEME_STORAGE_KEY,
  UI_FONT_STORAGE_KEY,
  UI_FONT_WEIGHT_STORAGE_KEY,
  applyColorSchemeToDocument,
  applyContentFontToDocument,
  applyContentFontWeightToDocument,
  applyFontWeightToDocument,
  applyIntensityToDocument,
  applyPlatformToDocument,
  applyRailAppearanceToDocument,
  applyReducedMotionToDocument,
  applySurfaceToDocument,
  applyThemeToDocument,
  applyUiFontToDocument,
  isColorScheme,
  isContentFont,
  isIntensity,
  isFontWeight,
  isRailAppearance,
  isThemeMode,
  isThemePreference,
  isUiFont,
  parseColorSchemePair,
  readDocumentColorScheme,
  readDocumentContentFont,
  readDocumentContentFontWeight,
  readDocumentFontWeight,
  readDocumentIntensity,
  readDocumentRailAppearance,
  readDocumentReducedMotion,
  readDocumentSurface,
  readDocumentUiFont,
  readDocumentTheme,
  readSystemPlatform,
  readSystemTheme,
  resolveThemePreference,
  type ColorScheme,
  type ColorSchemePair,
  type ContentFont,
  type FontWeight,
  type Intensity,
  type RailAppearance,
  type ReducedMotionPreference,
  type Surface,
  type UiFont,
  type ThemeMode,
  type ThemePreference,
} from '../theme'

/**
 * Appearance state, shared by every component that asks for it.
 *
 * Module-scoped rather than per-component: there is one `<html>` element, so
 * there is one theme. A `provide`/`inject` pair would be more idiomatic Vue and
 * would also let two subtrees disagree about a value that is physically global.
 */
const theme = ref<ThemeMode>(DEFAULT_THEME)
/** `system` is a preference, not a palette; `theme` above is always resolved. */
const themePreference = ref<ThemePreference>('system')
/**
 * The hue each theme is set in.
 *
 * The pair is the state and `colorScheme` below is the half in force, so a
 * component that only knows about "the accent" keeps working while the two
 * themes stop having to agree.
 */
const colorSchemes = ref<ColorSchemePair>({
  light: DEFAULT_COLOR_SCHEME,
  dark: DEFAULT_COLOR_SCHEME,
})
const intensity = ref<Intensity>(DEFAULT_INTENSITY)
/** `null` means the operating system decides. */
const reducedMotion = ref<ReducedMotionPreference>(null)
const surface = ref<Surface>(DEFAULT_SURFACE)
const uiFont = ref<UiFont>(DEFAULT_UI_FONT)
const contentFont = ref<ContentFont>(DEFAULT_CONTENT_FONT)
const railAppearance = ref<RailAppearance>(DEFAULT_RAIL_APPEARANCE)
const uiFontWeight = ref<FontWeight>(DEFAULT_FONT_WEIGHT)
const contentFontWeight = ref<FontWeight>(DEFAULT_FONT_WEIGHT)

/** The version of the profile payload `exportAppearanceProfile` writes. */
export const APPEARANCE_PROFILE_VERSION = 2

export interface AppearanceProfile {
  version: number
  /** Absent or `system` both mean "follow the operating system". */
  theme?: ThemeMode | undefined
  /**
   * The hue both themes share.
   *
   * Written when the two agree, so a reader of the older profile still gets the
   * reader's colour; absent when they differ, because one hue cannot say two.
   */
  colorScheme?: ColorScheme
  /** The hue each theme is set in. */
  colorSchemes?: ColorSchemePair
  intensity?: Intensity
  uiFont?: UiFont
  contentFont?: ContentFont
  railAppearance?: RailAppearance
  uiFontWeight?: FontWeight
  contentFontWeight?: FontWeight
}

let initialized = false
let stopWatchingSystem: (() => void) | null = null

/** The hue the theme in force is set in. */
const colorScheme = computed(() => colorSchemes.value[theme.value])

function setTheme(next: ThemeMode): void {
  theme.value = next
  applyThemeToDocument(next)
  // A theme change can be a hue change as well: the reader is allowed to set
  // the two themes to different accents, so the attribute has to follow the
  // theme rather than sit at whatever was chosen last.
  applyColorSchemeToDocument(colorSchemes.value[next])
}

/** Re-resolve `system` against whatever the OS is asking for right now. */
function applyThemePreference(): void {
  setTheme(resolveThemePreference(themePreference.value, readSystemTheme()))
}

function persistColorSchemes(): void {
  writeStored(COLOR_SCHEMES_STORAGE_KEY, JSON.stringify(colorSchemes.value))
}

/**
 * Set the hue for both themes, which is what a single-axis choice means.
 *
 * Kept because it is the shape the public API and the profiles already had: a
 * caller that knows about one accent is saying "use this one", not "use this
 * one on the theme I happen to be in".
 */
function setColorScheme(next: ColorScheme): void {
  colorSchemes.value = { light: next, dark: next }
  persistColorSchemes()
  applyColorSchemeToDocument(colorSchemes.value[theme.value])
}

/** Set the hue for one of the two themes. */
function setColorSchemeFor(mode: ThemeMode, next: ColorScheme): void {
  colorSchemes.value = { ...colorSchemes.value, [mode]: next }
  persistColorSchemes()
  if (mode === theme.value) applyColorSchemeToDocument(next)
}

/**
 * Follow the OS while the user has no stored preference.
 *
 * Registered even when a `data-theme` was pre-applied by the inline head script,
 * because that script's value came from the same three sources in the same order.
 */
function watchSystemTheme(): void {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return
  const query = window.matchMedia('(prefers-color-scheme: light)')
  const onChange = (event: MediaQueryListEvent): void => {
    if (themePreference.value !== 'system') return
    setTheme(event.matches ? 'light' : 'dark')
  }
  query.addEventListener('change', onChange)
  stopWatchingSystem = () => query.removeEventListener('change', onChange)
}

/**
 * Resolve and apply the initial appearance.
 *
 * Precedence: what the user stored, then what an inline head script already put
 * on the element, then the OS, then the default. The middle step matters — it is
 * how the first paint and the hydrated app agree, instead of the app "correcting"
 * a correct value and flashing.
 */
export function initAppearance(): {
  theme: ThemeMode
  colorScheme: ColorScheme
  intensity: Intensity
  reducedMotion: ReducedMotionPreference
  surface: Surface
  uiFont: UiFont
  contentFont: ContentFont
  railAppearance: RailAppearance
  uiFontWeight: FontWeight
  contentFontWeight: FontWeight
} {
  const storedTheme = readStored(THEME_STORAGE_KEY)
  const storedScheme = readStored(COLOR_SCHEME_STORAGE_KEY)
  const storedSchemePair = parseColorSchemePair(readStored(COLOR_SCHEMES_STORAGE_KEY))
  const storedIntensity = readStored(INTENSITY_STORAGE_KEY)
  const storedReducedMotion = readStored(REDUCED_MOTION_STORAGE_KEY)
  const storedUiFont = readStored(UI_FONT_STORAGE_KEY)
  const storedContentFont = readStored(CONTENT_FONT_STORAGE_KEY)
  const storedRailAppearance = readStored(RAIL_APPEARANCE_STORAGE_KEY)
  const storedUiFontWeight = readStored(UI_FONT_WEIGHT_STORAGE_KEY)
  const storedContentFontWeight = readStored(CONTENT_FONT_WEIGHT_STORAGE_KEY)

  // A stored `light`/`dark` from before this axis existed is still an explicit
  // choice, so it is read as one rather than coerced to `system`.
  if (isThemePreference(storedTheme)) themePreference.value = storedTheme
  else themePreference.value = isThemeMode(storedTheme) ? storedTheme : 'system'

  // A data-theme already on the element came from the bootstrap script, which
  // resolved the same inputs against the same OS. Trust it over a fresh reading,
  // or the hydrated app "corrects" a correct value and flashes.
  setTheme(
    themePreference.value === 'system'
      ? (readDocumentTheme() ?? readSystemTheme() ?? DEFAULT_THEME)
      : themePreference.value,
  )
  // The pair is read per theme; the legacy single key and an attribute already
  // on the element both speak for whichever theme the reader never separated.
  const documentScheme = readDocumentColorScheme() ?? DEFAULT_COLOR_SCHEME
  const sharedScheme = isColorScheme(storedScheme) ? storedScheme : documentScheme
  colorSchemes.value = {
    light: storedSchemePair.light ?? sharedScheme,
    dark: storedSchemePair.dark ?? sharedScheme,
  }
  applyColorSchemeToDocument(colorSchemes.value[theme.value])

  intensity.value = isIntensity(storedIntensity)
    ? storedIntensity
    : (readDocumentIntensity() ?? DEFAULT_INTENSITY)
  applyIntensityToDocument(intensity.value)

  const documentReducedMotion = readDocumentReducedMotion()
  reducedMotion.value =
    storedReducedMotion === 'true' || storedReducedMotion === 'false'
      ? storedReducedMotion
      : documentReducedMotion
  applyReducedMotionToDocument(reducedMotion.value)

  // The surface is announced by the host rather than stored, so a pre-set
  // attribute is the only real input and the default is the fallback.
  surface.value = readDocumentSurface() ?? DEFAULT_SURFACE
  applySurfaceToDocument(surface.value)
  applyPlatformToDocument(readSystemPlatform())

  // Fonts and the rail's opacity are tastes rather than environment facts, so
  // an attribute already on the element (written by the bootstrap script from
  // the same storage) is as good an input as the storage itself.
  uiFont.value = isUiFont(storedUiFont)
    ? storedUiFont
    : (readDocumentUiFont() ?? DEFAULT_UI_FONT)
  applyUiFontToDocument(uiFont.value)

  contentFont.value = isContentFont(storedContentFont)
    ? storedContentFont
    : (readDocumentContentFont() ?? DEFAULT_CONTENT_FONT)
  applyContentFontToDocument(contentFont.value)

  railAppearance.value = isRailAppearance(storedRailAppearance)
    ? storedRailAppearance
    : (readDocumentRailAppearance() ?? DEFAULT_RAIL_APPEARANCE)
  applyRailAppearanceToDocument(railAppearance.value)

  uiFontWeight.value = isFontWeight(storedUiFontWeight)
    ? storedUiFontWeight
    : (readDocumentFontWeight() ?? DEFAULT_FONT_WEIGHT)
  applyFontWeightToDocument(uiFontWeight.value)

  contentFontWeight.value = isFontWeight(storedContentFontWeight)
    ? storedContentFontWeight
    : (readDocumentContentFontWeight() ?? DEFAULT_FONT_WEIGHT)
  applyContentFontWeightToDocument(contentFontWeight.value)

  if (!stopWatchingSystem) watchSystemTheme()
  initialized = true
  return {
    theme: theme.value,
    colorScheme: colorScheme.value,
    intensity: intensity.value,
    reducedMotion: reducedMotion.value,
    surface: surface.value,
    uiFont: uiFont.value,
    contentFont: contentFont.value,
    railAppearance: railAppearance.value,
    uiFontWeight: uiFontWeight.value,
    contentFontWeight: contentFontWeight.value,
  }
}

/** Reset the module singleton. Exists for tests, which need a clean document. */
export function resetAppearanceForTests(): void {
  initialized = false
  themePreference.value = 'system'
  stopWatchingSystem?.()
  stopWatchingSystem = null
  theme.value = DEFAULT_THEME
  colorSchemes.value = { light: DEFAULT_COLOR_SCHEME, dark: DEFAULT_COLOR_SCHEME }
  intensity.value = DEFAULT_INTENSITY
  reducedMotion.value = null
  surface.value = DEFAULT_SURFACE
  uiFont.value = DEFAULT_UI_FONT
  contentFont.value = DEFAULT_CONTENT_FONT
  railAppearance.value = DEFAULT_RAIL_APPEARANCE
  uiFontWeight.value = DEFAULT_FONT_WEIGHT
  contentFontWeight.value = DEFAULT_FONT_WEIGHT
}

/**
 * A profile is the set of choices, named as values rather than as storage keys.
 *
 * Storage keys are a local detail — they can be renamed, and two builds can
 * disagree about them — while the profile is the thing a user hands to someone
 * else or keeps in a dotfile.
 */
export function exportAppearanceProfile(): AppearanceProfile {
  const pair = colorSchemes.value
  return {
    version: APPEARANCE_PROFILE_VERSION,
    theme: themePreference.value === 'system' ? undefined : themePreference.value,
    colorSchemes: { ...pair },
    ...(pair.light === pair.dark ? { colorScheme: pair.dark } : {}),
    intensity: intensity.value,
    uiFont: uiFont.value,
    contentFont: contentFont.value,
    railAppearance: railAppearance.value,
    uiFontWeight: uiFontWeight.value,
    contentFontWeight: contentFontWeight.value,
  }
}

/**
 * Apply a profile, or refuse it whole.
 *
 * A profile arrives from a file or another machine, so it is validated before
 * anything is written: half-applying a broken one would leave the user with a
 * mixture of two themes and no way to tell which choice came from where.
 */
export function importAppearanceProfile(candidate: unknown): boolean {
  if (!candidate || typeof candidate !== 'object') return false
  const profile = candidate as Record<string, unknown>

  const profileTheme = profile.theme === undefined ? 'system' : profile.theme
  if (!isThemePreference(profileTheme)) return false
  if (profile.colorScheme !== undefined && !isColorScheme(profile.colorScheme)) return false
  const pairCandidate =
    profile.colorSchemes === undefined
      ? null
      : profile.colorSchemes && typeof profile.colorSchemes === 'object'
        ? (profile.colorSchemes as Record<string, unknown>)
        : 'not a pair'
  if (pairCandidate === 'not a pair') return false
  if (pairCandidate) {
    for (const half of ['light', 'dark'] as const) {
      const value = pairCandidate[half]
      if (value !== undefined && !isColorScheme(value)) return false
    }
  }
  if (profile.intensity !== undefined && !isIntensity(profile.intensity)) return false
  if (profile.uiFont !== undefined && !isUiFont(profile.uiFont)) return false
  if (profile.contentFont !== undefined && !isContentFont(profile.contentFont)) return false
  if (profile.railAppearance !== undefined && !isRailAppearance(profile.railAppearance)) return false
  if (profile.uiFontWeight !== undefined && !isFontWeight(profile.uiFontWeight)) return false
  if (profile.contentFontWeight !== undefined && !isFontWeight(profile.contentFontWeight)) return false

  if (profileTheme !== 'system') {
    themePreference.value = profileTheme
    setTheme(profileTheme)
    writeStored(THEME_STORAGE_KEY, profileTheme)
  }

  if (isColorScheme(profile.colorScheme)) {
    setColorScheme(profile.colorScheme)
    writeStored(COLOR_SCHEME_STORAGE_KEY, profile.colorScheme)
  }
  if (pairCandidate) {
    // A pair that names only one half leaves the other where it was, which is
    // what a profile written by a build with one hue per theme should do.
    const next: ColorSchemePair = { ...colorSchemes.value }
    if (isColorScheme(pairCandidate.light)) next.light = pairCandidate.light
    if (isColorScheme(pairCandidate.dark)) next.dark = pairCandidate.dark
    colorSchemes.value = next
    persistColorSchemes()
    applyColorSchemeToDocument(next[theme.value])
  }
  if (isIntensity(profile.intensity)) {
    intensity.value = profile.intensity
    applyIntensityToDocument(profile.intensity)
    writeStored(INTENSITY_STORAGE_KEY, profile.intensity)
  }
  if (isUiFont(profile.uiFont)) {
    uiFont.value = profile.uiFont
    applyUiFontToDocument(profile.uiFont)
    writeStored(UI_FONT_STORAGE_KEY, profile.uiFont)
  }
  if (isContentFont(profile.contentFont)) {
    contentFont.value = profile.contentFont
    applyContentFontToDocument(profile.contentFont)
    writeStored(CONTENT_FONT_STORAGE_KEY, profile.contentFont)
  }
  if (isRailAppearance(profile.railAppearance)) {
    railAppearance.value = profile.railAppearance
    applyRailAppearanceToDocument(profile.railAppearance)
    writeStored(RAIL_APPEARANCE_STORAGE_KEY, profile.railAppearance)
  }
  if (isFontWeight(profile.uiFontWeight)) {
    uiFontWeight.value = profile.uiFontWeight
    applyFontWeightToDocument(profile.uiFontWeight)
    writeStored(UI_FONT_WEIGHT_STORAGE_KEY, profile.uiFontWeight)
  }
  if (isFontWeight(profile.contentFontWeight)) {
    contentFontWeight.value = profile.contentFontWeight
    applyContentFontWeightToDocument(profile.contentFontWeight)
    writeStored(CONTENT_FONT_WEIGHT_STORAGE_KEY, profile.contentFontWeight)
  }

  return true
}

/** Put every axis back to its default and forget what was stored. */
export function resetAppearance(): void {
  for (const key of [
    THEME_STORAGE_KEY,
    COLOR_SCHEME_STORAGE_KEY,
    COLOR_SCHEMES_STORAGE_KEY,
    INTENSITY_STORAGE_KEY,
    REDUCED_MOTION_STORAGE_KEY,
    UI_FONT_STORAGE_KEY,
    CONTENT_FONT_STORAGE_KEY,
    RAIL_APPEARANCE_STORAGE_KEY,
    UI_FONT_WEIGHT_STORAGE_KEY,
    CONTENT_FONT_WEIGHT_STORAGE_KEY,
  ]) {
    clearStored(key)
  }

  themePreference.value = 'system'
  colorSchemes.value = { light: DEFAULT_COLOR_SCHEME, dark: DEFAULT_COLOR_SCHEME }
  applyColorSchemeToDocument(DEFAULT_COLOR_SCHEME)
  intensity.value = DEFAULT_INTENSITY
  applyIntensityToDocument(DEFAULT_INTENSITY)
  reducedMotion.value = null
  applyReducedMotionToDocument(null)
  uiFont.value = DEFAULT_UI_FONT
  applyUiFontToDocument(DEFAULT_UI_FONT)
  contentFont.value = DEFAULT_CONTENT_FONT
  applyContentFontToDocument(DEFAULT_CONTENT_FONT)
  railAppearance.value = DEFAULT_RAIL_APPEARANCE
  applyRailAppearanceToDocument(DEFAULT_RAIL_APPEARANCE)
  uiFontWeight.value = DEFAULT_FONT_WEIGHT
  applyFontWeightToDocument(DEFAULT_FONT_WEIGHT)
  contentFontWeight.value = DEFAULT_FONT_WEIGHT
  applyContentFontWeightToDocument(DEFAULT_FONT_WEIGHT)
  applyThemePreference()
}

export interface AppearanceApi {
  theme: Readonly<Ref<ThemeMode>>
  /** What the user asked for: `system`, `light` or `dark`. */
  themePreference: Readonly<Ref<ThemePreference>>
  /** The hue the theme in force is set in. */
  colorScheme: Readonly<Ref<ColorScheme>>
  /** The hue each of the two themes is set in. */
  colorSchemes: Readonly<Ref<ColorSchemePair>>
  intensity: Readonly<Ref<Intensity>>
  reducedMotion: Readonly<Ref<ReducedMotionPreference>>
  surface: Readonly<Ref<Surface>>
  uiFont: Readonly<Ref<UiFont>>
  contentFont: Readonly<Ref<ContentFont>>
  railAppearance: Readonly<Ref<RailAppearance>>
  uiFontWeight: Readonly<Ref<FontWeight>>
  contentFontWeight: Readonly<Ref<FontWeight>>
  isDark: ComputedRef<boolean>
  /** Whether motion should be suppressed right now, OS included. */
  prefersReducedMotion: ComputedRef<boolean>
  setTheme: (next: ThemeMode) => void
  setThemePreference: (next: ThemePreference) => void
  toggleTheme: () => void
  setColorScheme: (next: ColorScheme) => void
  /** Set the hue for one of the two themes, leaving the other alone. */
  setColorSchemeFor: (mode: ThemeMode, next: ColorScheme) => void
  setIntensity: (next: Intensity) => void
  setReducedMotion: (next: ReducedMotionPreference) => void
  setSurface: (next: Surface) => void
  setUiFont: (next: UiFont) => void
  setContentFont: (next: ContentFont) => void
  setRailAppearance: (next: RailAppearance) => void
  setUiFontWeight: (next: FontWeight) => void
  setContentFontWeight: (next: FontWeight) => void
}

export function useAppearance(): AppearanceApi {
  if (!initialized) initAppearance()

  return {
    theme: readonly(theme),
    themePreference: readonly(themePreference),
    colorScheme: computed(() => colorScheme.value),
    colorSchemes: readonly(colorSchemes),
    intensity: readonly(intensity),
    reducedMotion: readonly(reducedMotion),
    surface: readonly(surface),
    uiFont: readonly(uiFont),
    contentFont: readonly(contentFont),
    railAppearance: readonly(railAppearance),
    uiFontWeight: readonly(uiFontWeight),
    contentFontWeight: readonly(contentFontWeight),
    isDark: computed(() => theme.value === 'dark'),
    prefersReducedMotion: computed(() => {
      if (reducedMotion.value === 'true') return true
      if (reducedMotion.value === 'false') return false
      if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false
      return window.matchMedia('(prefers-reduced-motion: reduce)').matches
    }),
    setTheme: (next: ThemeMode) => {
      themePreference.value = next
      setTheme(next)
      writeStored(THEME_STORAGE_KEY, next)
    },
    setThemePreference: (next: ThemePreference) => {
      themePreference.value = next
      applyThemePreference()
      writeStored(THEME_STORAGE_KEY, next)
    },
    toggleTheme: () => {
      const next: ThemeMode = theme.value === 'dark' ? 'light' : 'dark'
      themePreference.value = next
      setTheme(next)
      writeStored(THEME_STORAGE_KEY, next)
    },
    setColorScheme: (next: ColorScheme) => {
      setColorScheme(next)
      writeStored(COLOR_SCHEME_STORAGE_KEY, next)
    },
    setColorSchemeFor: (mode: ThemeMode, next: ColorScheme) => {
      setColorSchemeFor(mode, next)
    },
    setIntensity: (next: Intensity) => {
      intensity.value = next
      applyIntensityToDocument(next)
      writeStored(INTENSITY_STORAGE_KEY, next)
    },
    setReducedMotion: (next: ReducedMotionPreference) => {
      reducedMotion.value = next
      applyReducedMotionToDocument(next)
      // `null` clears the preference rather than storing a third value, so
      // "follow the OS" is reachable again after an explicit choice.
      if (next === null) clearStored(REDUCED_MOTION_STORAGE_KEY)
      else writeStored(REDUCED_MOTION_STORAGE_KEY, next)
    },
    setSurface: (next: Surface) => {
      // Deliberately not persisted: the surface describes the build, not a taste.
      surface.value = next
      applySurfaceToDocument(next)
    },
    setUiFont: (next: UiFont) => {
      uiFont.value = next
      applyUiFontToDocument(next)
      writeStored(UI_FONT_STORAGE_KEY, next)
    },
    setContentFont: (next: ContentFont) => {
      contentFont.value = next
      applyContentFontToDocument(next)
      writeStored(CONTENT_FONT_STORAGE_KEY, next)
    },
    setRailAppearance: (next: RailAppearance) => {
      railAppearance.value = next
      applyRailAppearanceToDocument(next)
      writeStored(RAIL_APPEARANCE_STORAGE_KEY, next)
    },
    setUiFontWeight: (next: FontWeight) => {
      uiFontWeight.value = next
      applyFontWeightToDocument(next)
      writeStored(UI_FONT_WEIGHT_STORAGE_KEY, next)
    },
    setContentFontWeight: (next: FontWeight) => {
      contentFontWeight.value = next
      applyContentFontWeightToDocument(next)
      writeStored(CONTENT_FONT_WEIGHT_STORAGE_KEY, next)
    },
  }
}
