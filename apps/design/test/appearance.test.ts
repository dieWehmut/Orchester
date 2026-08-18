import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  COLOR_SCHEME_ATTRIBUTE,
  COLOR_SCHEME_STORAGE_KEY,
  THEME_ATTRIBUTE,
  THEME_STORAGE_KEY,
  initAppearance,
  resetAppearanceForTests,
  useAppearance,
  type ThemeMode,
} from '../src'

/**
 * A `matchMedia` we can steer.
 *
 * jsdom ships one, but it answers `false` to every query and never emits a
 * change, so with the real implementation the OS-follows and
 * OS-stops-following branches are both unreachable — the two branches most
 * likely to be got wrong.
 */
type ChangeListener = (event: MediaQueryListEvent) => void

interface FakeQuery {
  matches: boolean
  listeners: Set<ChangeListener>
}

const LIGHT_QUERY = '(prefers-color-scheme: light)'

const realMatchMedia = window.matchMedia
const queries = new Map<string, FakeQuery>()

function stubMatchMedia(systemTheme: ThemeMode | null): void {
  queries.clear()
  window.matchMedia = ((media: string): MediaQueryList => {
    let query = queries.get(media)
    if (!query) {
      query = {
        matches: media.includes('light') ? systemTheme === 'light' : systemTheme === 'dark',
        listeners: new Set<ChangeListener>(),
      }
      queries.set(media, query)
    }
    const bound = query
    return {
      media,
      get matches() {
        return bound.matches
      },
      onchange: null,
      addEventListener: (_type: string, listener: ChangeListener) => bound.listeners.add(listener),
      removeEventListener: (_type: string, listener: ChangeListener) =>
        bound.listeners.delete(listener),
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    } as unknown as MediaQueryList
  }) as typeof window.matchMedia
}

/** Pretend the operating system just switched theme. */
function emitSystemTheme(next: ThemeMode): void {
  const query = queries.get(LIGHT_QUERY)
  if (!query) throw new Error(`nothing subscribed to ${LIGHT_QUERY}`)
  query.matches = next === 'light'
  for (const listener of query.listeners) {
    listener({ matches: query.matches } as MediaQueryListEvent)
  }
}

/**
 * Run `body` with a `localStorage` that throws on access.
 *
 * Defined as an own property on the global so deleting it afterwards exposes
 * jsdom's prototype accessor again, rather than leaving the suite storage-less.
 */
function withThrowingStorage(body: () => void): void {
  const originalDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    get() {
      throw new Error('SecurityError: site data is blocked for this origin')
    },
  })
  try {
    body()
  } finally {
    if (originalDescriptor) {
      Object.defineProperty(globalThis, 'localStorage', originalDescriptor)
    } else {
      Reflect.deleteProperty(globalThis, 'localStorage')
    }
  }
}

function root(): HTMLElement {
  return document.documentElement
}

beforeEach(() => {
  localStorage.clear()
  root().removeAttribute(THEME_ATTRIBUTE)
  root().removeAttribute(COLOR_SCHEME_ATTRIBUTE)
  root().style.removeProperty('color-scheme')
  resetAppearanceForTests()
  stubMatchMedia(null)
})

afterEach(() => {
  resetAppearanceForTests()
  window.matchMedia = realMatchMedia
})

describe('initAppearance', () => {
  it('applies both axes to the document', () => {
    const applied = initAppearance()

    expect(applied).toEqual({ theme: 'dark', colorScheme: 'amber' })
    expect(root().getAttribute(THEME_ATTRIBUTE)).toBe('dark')
    expect(root().getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('amber')
  })

  it('mirrors the theme onto the color-scheme property so browser UI follows', () => {
    initAppearance()

    expect(root().style.colorScheme).toBe('dark')
  })

  it('prefers a stored theme over the operating system', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'light')
    stubMatchMedia('dark')

    expect(initAppearance().theme).toBe('light')
  })

  it('prefers an attribute already on the element over the operating system', () => {
    // What an inline <head> script leaves behind. Re-deriving the theme here and
    // landing somewhere else is exactly the flash the script exists to prevent.
    root().setAttribute(THEME_ATTRIBUTE, 'light')
    stubMatchMedia('dark')

    expect(initAppearance().theme).toBe('light')
  })

  it('falls back to the operating system when the user has stored nothing', () => {
    stubMatchMedia('light')

    expect(initAppearance().theme).toBe('light')
  })

  it('ignores a stored value that is not one of the axes', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'midnight')
    localStorage.setItem(COLOR_SCHEME_STORAGE_KEY, 'chartreuse')

    expect(initAppearance()).toEqual({ theme: 'dark', colorScheme: 'amber' })
  })

  it('survives a localStorage that throws on access', () => {
    withThrowingStorage(() => {
      expect(() => initAppearance()).not.toThrow()
      expect(root().getAttribute(THEME_ATTRIBUTE)).toBe('dark')
    })
  })
})

describe('useAppearance', () => {
  it('initialises on first use', () => {
    const { theme } = useAppearance()

    expect(theme.value).toBe('dark')
    expect(root().getAttribute(THEME_ATTRIBUTE)).toBe('dark')
  })

  it('toggles, persists, and exposes isDark', () => {
    const { isDark, toggleTheme, theme } = useAppearance()
    expect(isDark.value).toBe(true)

    toggleTheme()

    expect(theme.value).toBe('light')
    expect(isDark.value).toBe(false)
    expect(root().getAttribute(THEME_ATTRIBUTE)).toBe('light')
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('light')
  })

  it('follows the operating system until the user chooses', () => {
    stubMatchMedia('light')
    initAppearance()

    emitSystemTheme('dark')

    expect(root().getAttribute(THEME_ATTRIBUTE)).toBe('dark')
  })

  it('stops following the operating system once the user has chosen', () => {
    stubMatchMedia('light')
    const { setTheme, theme } = useAppearance()

    setTheme('dark')
    emitSystemTheme('light')

    // Someone who wants a dark editor on a light desktop keeps it at dawn.
    expect(theme.value).toBe('dark')
    expect(root().getAttribute(THEME_ATTRIBUTE)).toBe('dark')
  })

  it('writes the accent axis without touching the theme axis', () => {
    const { setColorScheme, colorScheme, theme } = useAppearance()

    setColorScheme('teal')

    expect(colorScheme.value).toBe('teal')
    expect(theme.value).toBe('dark')
    expect(root().getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('teal')
    expect(localStorage.getItem(COLOR_SCHEME_STORAGE_KEY)).toBe('teal')
  })

  it('keeps a preference set through a throwing localStorage in memory', () => {
    withThrowingStorage(() => {
      const { setColorScheme, colorScheme } = useAppearance()

      setColorScheme('rose')

      expect(colorScheme.value).toBe('rose')
      expect(root().getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('rose')
    })
  })
})

describe('resetAppearanceForTests', () => {
  it('unsubscribes from the system theme', () => {
    stubMatchMedia('dark')
    initAppearance()
    expect(queries.get(LIGHT_QUERY)?.listeners.size).toBe(1)

    resetAppearanceForTests()

    expect(queries.get(LIGHT_QUERY)?.listeners.size).toBe(0)
  })
})
