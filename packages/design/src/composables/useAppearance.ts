import { computed, readonly, ref, type ComputedRef, type Ref } from 'vue'

import { clearStored, readStored, writeStored } from '../storage'
import {
  COLOR_SCHEME_STORAGE_KEY,
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
const colorScheme = ref<ColorScheme>(DEFAULT_COLOR_SCHEME)
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
export const APPEARANCE_PROFILE_VERSION = 1

export interface AppearanceProfile {
  version: number
  /** Absent or `system` both mean "follow the operating system". */
  theme?: ThemeMode | undefined
  colorScheme?: ColorScheme
  intensity?: Intensity
  uiFont?: UiFont
  contentFont?: ContentFont
  railAppearance?: RailAppearance
  uiFontWeight?: FontWeight
  contentFontWeight?: FontWeight
}

let initialized = false
let stopWatchingSystem: (() => void) | null = null

function setTheme(next: ThemeMode): void {
  theme.value = next
  applyThemeToDocument(next)
}

/** Re-resolve `system` against whatever the OS is asking for right now. */
function applyThemePreference(): void {
  setTheme(resolveThemePreference(themePreference.value, readSystemTheme()))
}

function setColorScheme(next: ColorScheme): void {
  colorScheme.value = next
  applyColorSchemeToDocument(next)
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
  setColorScheme(
    isColorScheme(storedScheme)
      ? storedScheme
      : (readDocumentColorScheme() ?? DEFAULT_COLOR_SCHEME),
  )

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
  colorScheme.value = DEFAULT_COLOR_SCHEME
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
  return {
    version: APPEARANCE_PROFILE_VERSION,
    theme: themePreference.value === 'system' ? undefined : themePreference.value,
    colorScheme: colorScheme.value,
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

  const theme = profile.theme === undefined ? 'system' : profile.theme
  if (!isThemePreference(theme)) return false
  if (profile.colorScheme !== undefined && !isColorScheme(profile.colorScheme)) return false
  if (profile.intensity !== undefined && !isIntensity(profile.intensity)) return false
  if (profile.uiFont !== undefined && !isUiFont(profile.uiFont)) return false
  if (profile.contentFont !== undefined && !isContentFont(profile.contentFont)) return false
  if (profile.railAppearance !== undefined && !isRailAppearance(profile.railAppearance)) return false
  if (profile.uiFontWeight !== undefined && !isFontWeight(profile.uiFontWeight)) return false
  if (profile.contentFontWeight !== undefined && !isFontWeight(profile.contentFontWeight)) return false

  if (theme !== 'system') {
    themePreference.value = theme
    setTheme(theme)
    writeStored(THEME_STORAGE_KEY, theme)
  }

  if (isColorScheme(profile.colorScheme)) {
    setColorScheme(profile.colorScheme)
    writeStored(COLOR_SCHEME_STORAGE_KEY, profile.colorScheme)
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
  setColorScheme(DEFAULT_COLOR_SCHEME)
  colorScheme.value = DEFAULT_COLOR_SCHEME
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
  colorScheme: Readonly<Ref<ColorScheme>>
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
    colorScheme: readonly(colorScheme),
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
