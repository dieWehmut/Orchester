import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  APPEARANCE_BOOTSTRAP_SCRIPT,
  COLOR_SCHEME_ATTRIBUTE,
  COLOR_SCHEME_STORAGE_KEY,
  THEME_ATTRIBUTE,
  THEME_STORAGE_KEY,
} from '../src'

const originalMatchMedia = window.matchMedia

function runBootstrap(): void {
  window.eval(APPEARANCE_BOOTSTRAP_SCRIPT)
}

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

beforeEach(() => {
  localStorage.clear()
  document.documentElement.removeAttribute(THEME_ATTRIBUTE)
  document.documentElement.removeAttribute(COLOR_SCHEME_ATTRIBUTE)
  document.documentElement.style.removeProperty('color-scheme')
  stubSystemTheme('dark')
})

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

describe('APPEARANCE_BOOTSTRAP_SCRIPT', () => {
  it('applies stored appearance before the application starts', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'light')
    localStorage.setItem(COLOR_SCHEME_STORAGE_KEY, 'teal')

    runBootstrap()

    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('light')
    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('teal')
    expect(document.documentElement.style.colorScheme).toBe('light')
  })

  it('uses the operating system and defaults when nothing is stored', () => {
    stubSystemTheme('light')

    runBootstrap()

    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('light')
    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('amber')
  })

  it('keeps booting when storage access is blocked', () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      get() {
        throw new Error('SecurityError')
      },
    })

    try {
      expect(() => runBootstrap()).not.toThrow()
      expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('dark')
      expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('amber')
    } finally {
      if (descriptor) Object.defineProperty(globalThis, 'localStorage', descriptor)
    }
  })
})
