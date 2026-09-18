import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  APPEARANCE_BOOTSTRAP_SCRIPT,
  DEFAULT_CONTENT_FONT,
  DEFAULT_RAIL_APPEARANCE,
  DEFAULT_UI_FONT,
  RAIL_APPEARANCE_ATTRIBUTE,
  RAIL_APPEARANCE_VALUES,
  UI_FONT_ATTRIBUTE,
  CONTENT_FONT_ATTRIBUTE,
  UI_FONT_STORAGE_KEY,
  CONTENT_FONT_STORAGE_KEY,
  RAIL_APPEARANCE_STORAGE_KEY,
  UI_FONTS,
  CONTENT_FONTS,
  FONT_WEIGHTS,
  DEFAULT_FONT_WEIGHT,
  UI_FONT_WEIGHT_ATTRIBUTE,
  CONTENT_FONT_WEIGHT_ATTRIBUTE,
  UI_FONT_WEIGHT_STORAGE_KEY,
  CONTENT_FONT_WEIGHT_STORAGE_KEY,
  applyContentFontWeightToDocument,
  applyFontWeightToDocument,
  isFontWeight,
  readDocumentContentFontWeight,
  readDocumentFontWeight,
  applyContentFontToDocument,
  applyRailAppearanceToDocument,
  applyUiFontToDocument,
  initAppearance,
  isContentFont,
  isRailAppearance,
  isUiFont,
  readDocumentContentFont,
  readDocumentRailAppearance,
  readDocumentUiFont,
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
  UI_FONT_ATTRIBUTE,
  CONTENT_FONT_ATTRIBUTE,
  RAIL_APPEARANCE_ATTRIBUTE,
  UI_FONT_WEIGHT_ATTRIBUTE,
  CONTENT_FONT_WEIGHT_ATTRIBUTE,
]

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

beforeEach(() => {
  localStorage.clear()
  for (const attribute of MANAGED_ATTRIBUTES) {
    document.documentElement.removeAttribute(attribute)
  }
  stubSystemTheme('dark')
  resetAppearanceForTests()
})

describe('ui font axis', () => {
  it('offers exactly system, sans and serif, and guards against anything else', () => {
    expect([...UI_FONTS]).toEqual(['system', 'sans', 'serif'])
    expect(isUiFont('system')).toBe(true)
    expect(isUiFont('sans')).toBe(true)
    expect(isUiFont('serif')).toBe(true)
    expect(isUiFont('comic')).toBe(false)
  })

  it('defaults to the system font', () => {
    expect(DEFAULT_UI_FONT).toBe('system')
  })

  it('round-trips through the document', () => {
    applyUiFontToDocument('serif')
    expect(document.documentElement.getAttribute(UI_FONT_ATTRIBUTE)).toBe('serif')
    expect(readDocumentUiFont()).toBe('serif')
  })

  it('reports null for an unknown document value', () => {
    document.documentElement.setAttribute(UI_FONT_ATTRIBUTE, 'papyrus')
    expect(readDocumentUiFont()).toBeNull()
  })
})

describe('content font axis', () => {
  it('offers exactly ui and mono', () => {
    expect([...CONTENT_FONTS]).toEqual(['ui', 'mono'])
    expect(isContentFont('ui')).toBe(true)
    expect(isContentFont('mono')).toBe(true)
    expect(isContentFont('serif')).toBe(false)
  })

  it('defaults to the mono content face', () => {
    expect(DEFAULT_CONTENT_FONT).toBe('mono')
  })

  it('resolves ui to the interface face rather than a hard-coded family', () => {
    applyContentFontToDocument('ui')
    expect(document.documentElement.getAttribute(CONTENT_FONT_ATTRIBUTE)).toBe('ui')
    expect(readDocumentContentFont()).toBe('ui')
  })
})

describe('rail appearance axis', () => {
  it('offers exactly solid and translucent', () => {
    expect([...RAIL_APPEARANCE_VALUES]).toEqual(['solid', 'translucent'])
    expect(isRailAppearance('solid')).toBe(true)
    expect(isRailAppearance('translucent')).toBe(true)
    expect(isRailAppearance('blurred')).toBe(false)
  })

  it('defaults to the solid rail', () => {
    expect(DEFAULT_RAIL_APPEARANCE).toBe('solid')
  })

  it('round-trips through the document', () => {
    applyRailAppearanceToDocument('translucent')
    expect(document.documentElement.getAttribute(RAIL_APPEARANCE_ATTRIBUTE)).toBe('translucent')
    expect(readDocumentRailAppearance()).toBe('translucent')
  })
})

describe('font weight axis', () => {
  it('offers exactly regular and medium, and guards against anything else', () => {
    expect([...FONT_WEIGHTS]).toEqual(['regular', 'medium'])
    expect(isFontWeight('regular')).toBe(true)
    expect(isFontWeight('medium')).toBe(true)
    expect(isFontWeight('black')).toBe(false)
  })

  it('defaults to the regular weight', () => {
    expect(DEFAULT_FONT_WEIGHT).toBe('regular')
  })

  it('round-trips through the document', () => {
    applyFontWeightToDocument('medium')
    expect(document.documentElement.getAttribute(UI_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
    expect(readDocumentFontWeight()).toBe('medium')

    applyContentFontWeightToDocument('medium')
    expect(document.documentElement.getAttribute(CONTENT_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
    expect(readDocumentContentFontWeight()).toBe('medium')
  })

  it('reports null for an unknown document value', () => {
    document.documentElement.setAttribute(UI_FONT_WEIGHT_ATTRIBUTE, 'ultra')
    expect(readDocumentFontWeight()).toBeNull()

    document.documentElement.setAttribute(CONTENT_FONT_WEIGHT_ATTRIBUTE, 'ultra')
    expect(readDocumentContentFontWeight()).toBeNull()
  })
})

describe('appearance api', () => {
  it('exposes the three axes and persists what it sets', () => {
    initAppearance()
    const appearance = useAppearance()

    appearance.setUiFont('serif')
    appearance.setContentFont('ui')
    appearance.setRailAppearance('translucent')
    appearance.setUiFontWeight('medium')
    appearance.setContentFontWeight('medium')

    expect(localStorage.getItem(UI_FONT_STORAGE_KEY)).toBe('serif')
    expect(localStorage.getItem(CONTENT_FONT_STORAGE_KEY)).toBe('ui')
    expect(localStorage.getItem(RAIL_APPEARANCE_STORAGE_KEY)).toBe('translucent')
    expect(localStorage.getItem(UI_FONT_WEIGHT_STORAGE_KEY)).toBe('medium')
    expect(localStorage.getItem(CONTENT_FONT_WEIGHT_STORAGE_KEY)).toBe('medium')
    expect(document.documentElement.getAttribute(UI_FONT_ATTRIBUTE)).toBe('serif')
    expect(document.documentElement.getAttribute(CONTENT_FONT_ATTRIBUTE)).toBe('ui')
    expect(document.documentElement.getAttribute(RAIL_APPEARANCE_ATTRIBUTE)).toBe('translucent')
    expect(document.documentElement.getAttribute(UI_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
    expect(document.documentElement.getAttribute(CONTENT_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
  })

  it('reads stored preferences on init', () => {
    localStorage.setItem(UI_FONT_STORAGE_KEY, 'sans')
    localStorage.setItem(CONTENT_FONT_STORAGE_KEY, 'ui')
    localStorage.setItem(RAIL_APPEARANCE_STORAGE_KEY, 'translucent')
    localStorage.setItem(UI_FONT_WEIGHT_STORAGE_KEY, 'medium')
    localStorage.setItem(CONTENT_FONT_WEIGHT_STORAGE_KEY, 'medium')

    initAppearance()

    expect(document.documentElement.getAttribute(UI_FONT_ATTRIBUTE)).toBe('sans')
    expect(document.documentElement.getAttribute(CONTENT_FONT_ATTRIBUTE)).toBe('ui')
    expect(document.documentElement.getAttribute(RAIL_APPEARANCE_ATTRIBUTE)).toBe('translucent')
    expect(document.documentElement.getAttribute(UI_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
    expect(document.documentElement.getAttribute(CONTENT_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
  })
})

describe('bootstrap script', () => {
  it('applies the stored font and rail axes before first paint', () => {
    localStorage.setItem(UI_FONT_STORAGE_KEY, 'serif')
    localStorage.setItem(CONTENT_FONT_STORAGE_KEY, 'ui')
    localStorage.setItem(RAIL_APPEARANCE_STORAGE_KEY, 'translucent')
    localStorage.setItem(UI_FONT_WEIGHT_STORAGE_KEY, 'medium')
    localStorage.setItem(CONTENT_FONT_WEIGHT_STORAGE_KEY, 'medium')

    runBootstrap()

    expect(document.documentElement.getAttribute(UI_FONT_ATTRIBUTE)).toBe('serif')
    expect(document.documentElement.getAttribute(CONTENT_FONT_ATTRIBUTE)).toBe('ui')
    expect(document.documentElement.getAttribute(RAIL_APPEARANCE_ATTRIBUTE)).toBe('translucent')
    expect(document.documentElement.getAttribute(UI_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
    expect(document.documentElement.getAttribute(CONTENT_FONT_WEIGHT_ATTRIBUTE)).toBe('medium')
  })

  it('falls back to the defaults when nothing is stored', () => {
    runBootstrap()

    expect(document.documentElement.getAttribute(UI_FONT_ATTRIBUTE)).toBe('system')
    expect(document.documentElement.getAttribute(CONTENT_FONT_ATTRIBUTE)).toBe('mono')
    expect(document.documentElement.getAttribute(RAIL_APPEARANCE_ATTRIBUTE)).toBe('solid')
    expect(document.documentElement.getAttribute(UI_FONT_WEIGHT_ATTRIBUTE)).toBe('regular')
    expect(document.documentElement.getAttribute(CONTENT_FONT_WEIGHT_ATTRIBUTE)).toBe('regular')
  })

  it('stays dependency-free so the shell can inline it', () => {
    expect(APPEARANCE_BOOTSTRAP_SCRIPT).not.toMatch(/\bimport\b/)
    expect(APPEARANCE_BOOTSTRAP_SCRIPT).not.toMatch(/\brequire\(/)
  })
})
