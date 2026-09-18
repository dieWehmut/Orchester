import { computed, readonly, ref, type ComputedRef, type Ref } from 'vue'

import { clearStored, readStored, writeStored } from '../storage'
import {
  COLOR_SCHEME_STORAGE_KEY,
  DEFAULT_COLOR_SCHEME,
  DEFAULT_INTENSITY,
  DEFAULT_SURFACE,
  DEFAULT_THEME,
  INTENSITY_STORAGE_KEY,
  REDUCED_MOTION_STORAGE_KEY,
  THEME_STORAGE_KEY,
  applyColorSchemeToDocument,
  applyIntensityToDocument,
  applyPlatformToDocument,
  applyReducedMotionToDocument,
  applySurfaceToDocument,
  applyThemeToDocument,
  isColorScheme,
  isIntensity,
  isThemeMode,
  readDocumentColorScheme,
  readDocumentIntensity,
  readDocumentReducedMotion,
  readDocumentSurface,
  readDocumentTheme,
  readSystemPlatform,
  readSystemTheme,
  type ColorScheme,
  type Intensity,
  type ReducedMotionPreference,
  type Surface,
  type ThemeMode,
} from '../theme'

/**
 * Appearance state, shared by every component that asks for it.
 *
 * Module-scoped rather than per-component: there is one `<html>` element, so
 * there is one theme. A `provide`/`inject` pair would be more idiomatic Vue and
 * would also let two subtrees disagree about a value that is physically global.
 */
const theme = ref<ThemeMode>(DEFAULT_THEME)
const colorScheme = ref<ColorScheme>(DEFAULT_COLOR_SCHEME)
const intensity = ref<Intensity>(DEFAULT_INTENSITY)
/** `null` means the operating system decides. */
const reducedMotion = ref<ReducedMotionPreference>(null)
const surface = ref<Surface>(DEFAULT_SURFACE)

let initialized = false
/** True once the user has chosen, after which the OS no longer overrides. */
let themeIsExplicit = false
let stopWatchingSystem: (() => void) | null = null

function setTheme(next: ThemeMode): void {
  theme.value = next
  applyThemeToDocument(next)
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
    if (themeIsExplicit) return
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
} {
  const storedTheme = readStored(THEME_STORAGE_KEY)
  const storedScheme = readStored(COLOR_SCHEME_STORAGE_KEY)
  const storedIntensity = readStored(INTENSITY_STORAGE_KEY)
  const storedReducedMotion = readStored(REDUCED_MOTION_STORAGE_KEY)

  themeIsExplicit = isThemeMode(storedTheme)

  setTheme(
    isThemeMode(storedTheme)
      ? storedTheme
      : (readDocumentTheme() ?? readSystemTheme() ?? DEFAULT_THEME),
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

  if (!stopWatchingSystem) watchSystemTheme()
  initialized = true
  return {
    theme: theme.value,
    colorScheme: colorScheme.value,
    intensity: intensity.value,
    reducedMotion: reducedMotion.value,
    surface: surface.value,
  }
}

/** Reset the module singleton. Exists for tests, which need a clean document. */
export function resetAppearanceForTests(): void {
  initialized = false
  themeIsExplicit = false
  stopWatchingSystem?.()
  stopWatchingSystem = null
  theme.value = DEFAULT_THEME
  colorScheme.value = DEFAULT_COLOR_SCHEME
  intensity.value = DEFAULT_INTENSITY
  reducedMotion.value = null
  surface.value = DEFAULT_SURFACE
}

export interface AppearanceApi {
  theme: Readonly<Ref<ThemeMode>>
  colorScheme: Readonly<Ref<ColorScheme>>
  intensity: Readonly<Ref<Intensity>>
  reducedMotion: Readonly<Ref<ReducedMotionPreference>>
  surface: Readonly<Ref<Surface>>
  isDark: ComputedRef<boolean>
  /** Whether motion should be suppressed right now, OS included. */
  prefersReducedMotion: ComputedRef<boolean>
  setTheme: (next: ThemeMode) => void
  toggleTheme: () => void
  setColorScheme: (next: ColorScheme) => void
  setIntensity: (next: Intensity) => void
  setReducedMotion: (next: ReducedMotionPreference) => void
  setSurface: (next: Surface) => void
}

export function useAppearance(): AppearanceApi {
  if (!initialized) initAppearance()

  return {
    theme: readonly(theme),
    colorScheme: readonly(colorScheme),
    intensity: readonly(intensity),
    reducedMotion: readonly(reducedMotion),
    surface: readonly(surface),
    isDark: computed(() => theme.value === 'dark'),
    prefersReducedMotion: computed(() => {
      if (reducedMotion.value === 'true') return true
      if (reducedMotion.value === 'false') return false
      if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false
      return window.matchMedia('(prefers-reduced-motion: reduce)').matches
    }),
    setTheme: (next: ThemeMode) => {
      themeIsExplicit = true
      setTheme(next)
      writeStored(THEME_STORAGE_KEY, next)
    },
    toggleTheme: () => {
      const next: ThemeMode = theme.value === 'dark' ? 'light' : 'dark'
      themeIsExplicit = true
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
  }
}
