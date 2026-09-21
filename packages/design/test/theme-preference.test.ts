import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  APPEARANCE_BOOTSTRAP_SCRIPT,
  COLOR_SCHEME_ATTRIBUTE,
  DEFAULT_THEME,
  INTENSITY_ATTRIBUTE,
  THEME_ATTRIBUTE,
  THEME_PREFERENCES,
  THEME_STORAGE_KEY,
  initAppearance,
  isThemePreference,
  resetAppearanceForTests,
  useAppearance,
} from '../src'

const originalMatchMedia = window.matchMedia

function stubSystemTheme(theme: 'light' | 'dark'): void {
  window.matchMedia = ((query: string): MediaQueryList =>
    ({
      matches: query.includes(theme),
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    }) as MediaQueryList) as typeof window.matchMedia
}

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

beforeEach(() => {
  localStorage.clear()
  document.documentElement.removeAttribute(THEME_ATTRIBUTE)
  document.documentElement.removeAttribute(COLOR_SCHEME_ATTRIBUTE)
  document.documentElement.removeAttribute(INTENSITY_ATTRIBUTE)
  document.documentElement.style.removeProperty('color-scheme')
  stubSystemTheme('dark')
  resetAppearanceForTests()
})

describe('theme preference', () => {
  it('offers exactly system, light and dark', () => {
    expect([...THEME_PREFERENCES]).toEqual(['system', 'light', 'dark'])
    expect(isThemePreference('system')).toBe(true)
    expect(isThemePreference('light')).toBe(true)
    expect(isThemePreference('dark')).toBe(true)
    expect(isThemePreference('sepia')).toBe(false)
  })

  it('still resolves the document to one of the two themes', () => {
    // The attribute is what the stylesheet matches on; "system" is a preference,
    // not a third set of colours.
    const appearance = useAppearance()
    appearance.setThemePreference('system')

    expect([DEFAULT_THEME, 'light']).toContain(
      document.documentElement.getAttribute(THEME_ATTRIBUTE),
    )
    expect(appearance.theme.value).not.toBe('system')
  })

  it('follows the operating system while the preference is system', () => {
    stubSystemTheme('light')
    initAppearance()

    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('light')
  })

  it('reports the preference and the resolved theme separately', () => {
    const appearance = useAppearance()
    appearance.setThemePreference('system')

    expect(appearance.themePreference.value).toBe('system')
    expect(appearance.theme.value).toBe('dark')
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('system')
  })

  it('lets an explicit choice outrank a later operating-system change', () => {
    const appearance = useAppearance()
    appearance.setThemePreference('light')

    expect(appearance.themePreference.value).toBe('light')
    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('light')
  })

  it('tolerates a legacy stored light or dark value', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'light')

    expect(initAppearance().theme).toBe('light')
    expect(useAppearance().themePreference.value).toBe('light')
  })

  it('applies a stored system preference in the bootstrap script', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'system')
    stubSystemTheme('light')

    window.eval(APPEARANCE_BOOTSTRAP_SCRIPT)

    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('light')
  })
})
