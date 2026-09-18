import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  APPEARANCE_BOOTSTRAP_SCRIPT,
  COLOR_SCHEME_ATTRIBUTE,
  COLOR_SCHEME_STORAGE_KEY,
  DEFAULT_INTENSITY,
  INTENSITIES,
  INTENSITY_ATTRIBUTE,
  INTENSITY_STORAGE_KEY,
  OS_ATTRIBUTE,
  PLATFORMS,
  REDUCED_MOTION_ATTRIBUTE,
  REDUCED_MOTION_STORAGE_KEY,
  SURFACE_ATTRIBUTE,
  SURFACES,
  THEME_ATTRIBUTE,
  THEME_STORAGE_KEY,
  applyIntensityToDocument,
  applyPlatformToDocument,
  applyReducedMotionToDocument,
  applySurfaceToDocument,
  detectPlatform,
  initAppearance,
  isIntensity,
  isReducedMotionPreference,
  isSurface,
  readDocumentIntensity,
  readDocumentReducedMotion,
  readDocumentSurface,
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

function runBootstrap(): void {
  window.eval(APPEARANCE_BOOTSTRAP_SCRIPT)
}

const MANAGED_ATTRIBUTES = [
  THEME_ATTRIBUTE,
  COLOR_SCHEME_ATTRIBUTE,
  INTENSITY_ATTRIBUTE,
  REDUCED_MOTION_ATTRIBUTE,
  SURFACE_ATTRIBUTE,
  OS_ATTRIBUTE,
]

beforeEach(() => {
  localStorage.clear()
  for (const attribute of MANAGED_ATTRIBUTES) {
    document.documentElement.removeAttribute(attribute)
  }
  document.documentElement.style.removeProperty('color-scheme')
  stubSystemTheme('dark')
  resetAppearanceForTests()
})

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

describe('intensity axis', () => {
  it('offers exactly calm and vivid, and guards against anything else', () => {
    expect([...INTENSITIES]).toEqual(['calm', 'vivid'])
    expect(isIntensity('calm')).toBe(true)
    expect(isIntensity('vivid')).toBe(true)
    expect(isIntensity('loud')).toBe(false)
    expect(isIntensity(null)).toBe(false)
  })

  it('defaults to calm, the restrained treatment', () => {
    expect(DEFAULT_INTENSITY).toBe('calm')
  })

  it('round-trips through the document', () => {
    applyIntensityToDocument('vivid')
    expect(document.documentElement.getAttribute(INTENSITY_ATTRIBUTE)).toBe('vivid')
    expect(readDocumentIntensity()).toBe('vivid')
  })

  it('reports null when the document carries an unknown value', () => {
    document.documentElement.setAttribute(INTENSITY_ATTRIBUTE, 'shouting')
    expect(readDocumentIntensity()).toBeNull()
  })
})

describe('reduced-motion axis', () => {
  it('has three states, because "follow the OS" is not the same as "off"', () => {
    expect(isReducedMotionPreference('true')).toBe(true)
    expect(isReducedMotionPreference('false')).toBe(true)
    expect(isReducedMotionPreference(null)).toBe(true)
    expect(isReducedMotionPreference('maybe')).toBe(false)
  })

  it('writes the two explicit states', () => {
    applyReducedMotionToDocument('true')
    expect(document.documentElement.getAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe('true')
    applyReducedMotionToDocument('false')
    expect(document.documentElement.getAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe('false')
  })

  it('removes the attribute entirely when following the operating system', () => {
    applyReducedMotionToDocument('true')
    applyReducedMotionToDocument(null)
    expect(document.documentElement.hasAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe(false)
    expect(readDocumentReducedMotion()).toBeNull()
  })
})

describe('surface axis', () => {
  it('names the three faces the shell renders as', () => {
    expect([...SURFACES]).toEqual(['web', 'site', 'desktop'])
    expect(isSurface('desktop')).toBe(true)
    expect(isSurface('mobile')).toBe(false)
  })

  it('round-trips through the document', () => {
    applySurfaceToDocument('site')
    expect(document.documentElement.getAttribute(SURFACE_ATTRIBUTE)).toBe('site')
    expect(readDocumentSurface()).toBe('site')
  })
})

describe('platform detection', () => {
  it('recognises the three platforms the shell ships on', () => {
    expect([...PLATFORMS]).toEqual(['macos', 'windows', 'linux'])
    expect(
      detectPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'),
    ).toBe('macos')
    expect(
      detectPlatform('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'),
    ).toBe('windows')
    expect(
      detectPlatform('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'),
    ).toBe('linux')
  })

  it('falls back to linux rather than guessing a platform it cannot see', () => {
    expect(detectPlatform('')).toBe('linux')
  })

  it('writes the platform onto the document', () => {
    applyPlatformToDocument('windows')
    expect(document.documentElement.getAttribute(OS_ATTRIBUTE)).toBe('windows')
  })
})

describe('initAppearance', () => {
  it('sets every axis, not just the two it used to own', () => {
    localStorage.setItem(INTENSITY_STORAGE_KEY, 'vivid')
    localStorage.setItem(REDUCED_MOTION_STORAGE_KEY, 'true')

    initAppearance()

    expect(document.documentElement.getAttribute(INTENSITY_ATTRIBUTE)).toBe('vivid')
    expect(document.documentElement.getAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe('true')
    expect(document.documentElement.getAttribute(OS_ATTRIBUTE)).toBeTruthy()
  })

  it('defaults intensity to calm and reduced motion to the operating system', () => {
    initAppearance()

    expect(document.documentElement.getAttribute(INTENSITY_ATTRIBUTE)).toBe('calm')
    expect(document.documentElement.hasAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe(false)
  })

  it('prefers a surface the shell already announced over the web default', () => {
    document.documentElement.setAttribute(SURFACE_ATTRIBUTE, 'desktop')

    initAppearance()

    expect(document.documentElement.getAttribute(SURFACE_ATTRIBUTE)).toBe('desktop')
  })

  it('exposes every axis through the composable', () => {
    const appearance = useAppearance()
    appearance.setIntensity('vivid')
    appearance.setReducedMotion('false')
    appearance.setSurface('desktop')

    expect(appearance.intensity.value).toBe('vivid')
    expect(appearance.reducedMotion.value).toBe('false')
    expect(appearance.surface.value).toBe('desktop')
    expect(localStorage.getItem(INTENSITY_STORAGE_KEY)).toBe('vivid')
    expect(localStorage.getItem(REDUCED_MOTION_STORAGE_KEY)).toBe('false')
  })

  it('does not persist the surface: it is a property of the build, not a taste', () => {
    const appearance = useAppearance()
    appearance.setSurface('site')

    expect(localStorage.getItem(SURFACE_ATTRIBUTE)).toBeNull()
  })
})

describe('bootstrap script', () => {
  it('stays free of imports so it can run before any bundle loads', () => {
    expect(APPEARANCE_BOOTSTRAP_SCRIPT).not.toMatch(/\bimport\s|\brequire\(/)
  })

  it('applies the stored intensity and reduced motion before first paint', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'light')
    localStorage.setItem(COLOR_SCHEME_STORAGE_KEY, 'violet')
    localStorage.setItem(INTENSITY_STORAGE_KEY, 'vivid')
    localStorage.setItem(REDUCED_MOTION_STORAGE_KEY, 'true')

    runBootstrap()

    expect(document.documentElement.getAttribute(INTENSITY_ATTRIBUTE)).toBe('vivid')
    expect(document.documentElement.getAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe('true')
  })

  it('leaves reduced motion unset when the user follows the operating system', () => {
    runBootstrap()

    expect(document.documentElement.hasAttribute(REDUCED_MOTION_ATTRIBUTE)).toBe(false)
  })

  it('defaults intensity to calm and announces the platform', () => {
    runBootstrap()

    expect(document.documentElement.getAttribute(INTENSITY_ATTRIBUTE)).toBe('calm')
    expect(PLATFORMS).toContain(document.documentElement.getAttribute(OS_ATTRIBUTE))
  })

  it('keeps a surface the shell announced before the script ran', () => {
    document.documentElement.setAttribute(SURFACE_ATTRIBUTE, 'desktop')

    runBootstrap()

    expect(document.documentElement.getAttribute(SURFACE_ATTRIBUTE)).toBe('desktop')
  })
})