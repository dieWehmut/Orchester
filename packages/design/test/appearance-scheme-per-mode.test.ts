import { afterEach, beforeEach, describe, expect, it } from "vitest"

import {
  APPEARANCE_BOOTSTRAP_SCRIPT,
  COLOR_SCHEME_ATTRIBUTE,
  COLOR_SCHEME_STORAGE_KEY,
  COLOR_SCHEMES_STORAGE_KEY,
  THEME_ATTRIBUTE,
  THEME_STORAGE_KEY,
  exportAppearanceProfile,
  importAppearanceProfile,
  initAppearance,
  parseColorSchemePair,
  resetAppearanceForTests,
  resetAppearance,
  useAppearance,
} from "../src"

/**
 * The accent is chosen per theme, not once for both.
 *
 * The surface this design learned from offers a light theme and a dark theme
 * side by side, each with its own hue, because the accent that carries a light
 * page is rarely the one that carries a dark one. A single choice would make
 * the reader pick which of their two themes looks right.
 */

const originalMatchMedia = window.matchMedia

function stubSystemTheme(): void {
  window.matchMedia = ((query: string): MediaQueryList =>
    ({
      matches: query.includes("dark"),
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

beforeEach(() => {
  localStorage.clear()
  for (const attribute of document.documentElement.getAttributeNames()) {
    if (attribute.startsWith("data-")) document.documentElement.removeAttribute(attribute)
  }
  document.documentElement.style.removeProperty("color-scheme")
  stubSystemTheme()
  resetAppearanceForTests()
})

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

describe("per-mode colour scheme", () => {
  it("keeps a hue for each theme and applies the one in force", () => {
    const appearance = (initAppearance(), useAppearance())

    appearance.setColorSchemeFor("light", "teal")
    appearance.setColorSchemeFor("dark", "violet")

    expect(appearance.colorSchemes.value).toEqual({ light: "teal", dark: "violet" })

    // The system asks for dark above, so the dark hue is the applied one.
    expect(appearance.colorScheme.value).toBe("violet")
    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("violet")

    appearance.setTheme("light")

    expect(appearance.colorScheme.value).toBe("teal")
    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("teal")

    appearance.setTheme("dark")

    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("violet")
  })

  it("re-applies the right hue when the system drives a theme change", () => {
    const appearance = (initAppearance(), useAppearance())
    appearance.setColorSchemeFor("light", "codex")
    appearance.setColorSchemeFor("dark", "rose")
    appearance.setTheme("dark")

    // A reader on `system` whose desktop flips at dusk keeps both hues.
    appearance.setThemePreference("system")

    expect(appearance.colorScheme.value).toBe("rose")
    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("rose")
  })

  it("persists the pair, and reads a legacy single hue as both", () => {
    localStorage.setItem(COLOR_SCHEME_STORAGE_KEY, "teal")
    const appearance = (initAppearance(), useAppearance())

    expect(appearance.colorSchemes.value).toEqual({ light: "teal", dark: "teal" })

    appearance.setColorSchemeFor("dark", "codex")

    expect(appearance.colorSchemes.value).toEqual({ light: "teal", dark: "codex" })
    expect(localStorage.getItem(COLOR_SCHEMES_STORAGE_KEY)).toBe(
      JSON.stringify({ light: "teal", dark: "codex" }),
    )

    // A fresh start reads the pair back, with the legacy key still covering the
    // mode the reader never separated.
    resetAppearanceForTests()
    for (const attribute of document.documentElement.getAttributeNames()) {
      if (attribute.startsWith("data-")) document.documentElement.removeAttribute(attribute)
    }
    const restarted = (initAppearance(), useAppearance())

    expect(restarted.colorSchemes.value).toEqual({ light: "teal", dark: "codex" })
  })

  it("ignores a stored pair it cannot read, keeping the legacy hue", () => {
    localStorage.setItem(COLOR_SCHEME_STORAGE_KEY, "violet")
    localStorage.setItem(COLOR_SCHEMES_STORAGE_KEY, "{ not json")

    expect(parseColorSchemePair("{ not json")).toEqual({})
    expect(parseColorSchemePair(JSON.stringify({ light: "chartreuse", dark: "codex" }))).toEqual({
      dark: "codex",
    })

    const appearance = (initAppearance(), useAppearance())

    expect(appearance.colorSchemes.value).toEqual({ light: "violet", dark: "violet" })
  })

  it("carries both hues in the profile and accepts the single-hue form", () => {
    const appearance = (initAppearance(), useAppearance())
    appearance.setColorSchemeFor("light", "codex")
    appearance.setColorSchemeFor("dark", "teal")

    expect(exportAppearanceProfile().colorSchemes).toEqual({ light: "codex", dark: "teal" })

    // A profile written before this axis existed names one hue; it has to keep
    // importing, and the honest reading is that it means both themes.
    const applied = importAppearanceProfile({ version: 1, colorScheme: "violet" })

    expect(applied).toBe(true)
    expect(appearance.colorSchemes.value).toEqual({ light: "violet", dark: "violet" })

    const perMode = importAppearanceProfile({
      version: 1,
      colorSchemes: { light: "rose", dark: "codex" },
    })

    expect(perMode).toBe(true)
    expect(appearance.colorSchemes.value).toEqual({ light: "rose", dark: "codex" })

    expect(importAppearanceProfile({ version: 1, colorSchemes: { light: "chartreuse" } })).toBe(false)
  })

  it("resets both hues and forgets both keys", () => {
    const appearance = (initAppearance(), useAppearance())
    appearance.setColorSchemeFor("light", "teal")
    appearance.setColorSchemeFor("dark", "violet")

    resetAppearance()

    expect(appearance.colorSchemes.value).toEqual({ light: "rose", dark: "rose" })
    expect(localStorage.getItem(COLOR_SCHEMES_STORAGE_KEY)).toBeNull()
    expect(localStorage.getItem(COLOR_SCHEME_STORAGE_KEY)).toBeNull()
  })
})

describe("bootstrap script with per-mode hues", () => {
  it("paints the hue belonging to the theme it resolved", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "light")
    localStorage.setItem(
      COLOR_SCHEMES_STORAGE_KEY,
      JSON.stringify({ light: "violet", dark: "codex" }),
    )

    runBootstrap()

    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe("light")
    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("violet")

    localStorage.setItem(THEME_STORAGE_KEY, "dark")
    document.documentElement.removeAttribute(THEME_ATTRIBUTE)
    runBootstrap()

    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("codex")
  })

  it("falls back to the legacy single hue and then to the default", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "dark")
    localStorage.setItem(COLOR_SCHEME_STORAGE_KEY, "teal")
    localStorage.setItem(COLOR_SCHEMES_STORAGE_KEY, JSON.stringify({ light: "violet" }))

    runBootstrap()

    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe("teal")
  })
})