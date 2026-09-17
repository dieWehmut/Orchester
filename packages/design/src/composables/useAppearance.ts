import { computed, readonly, ref, type ComputedRef, type Ref } from 'vue'

import { readStored, writeStored } from '../storage'
import {
  COLOR_SCHEME_STORAGE_KEY,
  DEFAULT_COLOR_SCHEME,
  DEFAULT_THEME,
  THEME_STORAGE_KEY,
  applyColorSchemeToDocument,
  applyThemeToDocument,
  isColorScheme,
  isThemeMode,
  readDocumentColorScheme,
  readDocumentTheme,
  readSystemTheme,
  type ColorScheme,
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
export function initAppearance(): { theme: ThemeMode; colorScheme: ColorScheme } {
  const storedTheme = readStored(THEME_STORAGE_KEY)
  const storedScheme = readStored(COLOR_SCHEME_STORAGE_KEY)

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

  if (!stopWatchingSystem) watchSystemTheme()
  initialized = true
  return { theme: theme.value, colorScheme: colorScheme.value }
}

/** Reset the module singleton. Exists for tests, which need a clean document. */
export function resetAppearanceForTests(): void {
  initialized = false
  themeIsExplicit = false
  stopWatchingSystem?.()
  stopWatchingSystem = null
  theme.value = DEFAULT_THEME
  colorScheme.value = DEFAULT_COLOR_SCHEME
}

export interface AppearanceApi {
  theme: Readonly<Ref<ThemeMode>>
  colorScheme: Readonly<Ref<ColorScheme>>
  isDark: ComputedRef<boolean>
  setTheme: (next: ThemeMode) => void
  toggleTheme: () => void
  setColorScheme: (next: ColorScheme) => void
}

export function useAppearance(): AppearanceApi {
  if (!initialized) initAppearance()

  return {
    theme: readonly(theme),
    colorScheme: readonly(colorScheme),
    isDark: computed(() => theme.value === 'dark'),
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
  }
}
