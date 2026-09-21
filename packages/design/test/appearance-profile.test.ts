import { afterEach, beforeEach, describe, expect, it } from "vitest"

import {
  APPEARANCE_PROFILE_VERSION,
  CONTENT_FONT_STORAGE_KEY,
  CONTENT_FONT_WEIGHT_STORAGE_KEY,
  INTENSITY_STORAGE_KEY,
  RAIL_APPEARANCE_STORAGE_KEY,
  THEME_STORAGE_KEY,
  UI_FONT_STORAGE_KEY,
  UI_FONT_WEIGHT_STORAGE_KEY,
  COLOR_SCHEME_STORAGE_KEY,
  exportAppearanceProfile,
  importAppearanceProfile,
  initAppearance,
  resetAppearance,
  resetAppearanceForTests,
  useAppearance,
} from "../src"

const originalMatchMedia = window.matchMedia
const MANAGED_KEYS = [
  THEME_STORAGE_KEY,
  COLOR_SCHEME_STORAGE_KEY,
  INTENSITY_STORAGE_KEY,
  UI_FONT_STORAGE_KEY,
  CONTENT_FONT_STORAGE_KEY,
  RAIL_APPEARANCE_STORAGE_KEY,
  UI_FONT_WEIGHT_STORAGE_KEY,
  CONTENT_FONT_WEIGHT_STORAGE_KEY,
]

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

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

beforeEach(() => {
  localStorage.clear()
  // initAppearance prefers an attribute already on the element to avoid the
  // first-paint flash, so a leftover one from the previous test would win.
  for (const attribute of document.documentElement.getAttributeNames()) {
    if (attribute.startsWith("data-")) {
      document.documentElement.removeAttribute(attribute)
    }
  }
  document.documentElement.style.removeProperty("color-scheme")
  stubSystemTheme()
  resetAppearanceForTests()
})

describe("appearance profile", () => {
  it("exports a portable profile of every axis, not the storage keys", () => {
    const appearance = (initAppearance(), useAppearance())
    appearance.setTheme("light")
    appearance.setColorScheme("teal")
    appearance.setUiFont("serif")
    appearance.setUiFontWeight("medium")
    appearance.setRailAppearance("translucent")

    const profile = exportAppearanceProfile()

    expect(profile.version).toBe(APPEARANCE_PROFILE_VERSION)
    expect(profile.theme).toBe("light")
    expect(profile.colorScheme).toBe("teal")
    expect(profile.uiFont).toBe("serif")
    expect(profile.uiFontWeight).toBe("medium")
    expect(profile.railAppearance).toBe("translucent")
    // A profile travels between machines, so it names values rather than the
    // local storage layout, which is free to change underneath it.
    expect(Object.keys(profile)).not.toContain("orchester:theme")
  })

  it("imports a profile over the current session and persists it", () => {
    const appearance = (initAppearance(), useAppearance())

    const applied = importAppearanceProfile({
      version: APPEARANCE_PROFILE_VERSION,
      theme: "light",
      colorScheme: "violet",
      intensity: "calm",
      uiFont: "sans",
      contentFont: "ui",
      railAppearance: "translucent",
      uiFontWeight: "medium",
      contentFontWeight: "medium",
    })

    expect(applied).toBe(true)
    expect(appearance.theme.value).toBe("light")
    expect(appearance.colorScheme.value).toBe("violet")
    expect(localStorage.getItem(COLOR_SCHEME_STORAGE_KEY)).toBe("violet")
    expect(document.documentElement.getAttribute("data-color-scheme")).toBe("violet")
    expect(document.documentElement.getAttribute("data-ui-font")).toBe("sans")
    expect(document.documentElement.getAttribute("data-ui-font-weight")).toBe("medium")
  })

  it("refuses a profile it cannot read rather than half-applying it", () => {
    const appearance = (initAppearance(), useAppearance())
    appearance.setColorScheme("rose")

    const applied = importAppearanceProfile({
      version: APPEARANCE_PROFILE_VERSION,
      theme: "light",
      colorScheme: "chartreuse",
    } as never)

    expect(applied).toBe(false)
    expect(appearance.theme.value).toBe("dark")
    expect(appearance.colorScheme.value).toBe("rose")
  })

  it("refuses a payload that is not an object at all", () => {
    initAppearance()
    expect(importAppearanceProfile("not a profile" as never)).toBe(false)
    expect(importAppearanceProfile(null as never)).toBe(false)
  })

  it("resets every axis to its default and clears what was stored", () => {
    const appearance = (initAppearance(), useAppearance())
    appearance.setTheme("light")
    appearance.setColorScheme("teal")
    appearance.setUiFont("serif")
    appearance.setContentFont("ui")
    appearance.setRailAppearance("translucent")
    appearance.setUiFontWeight("medium")

    resetAppearance()

    expect(appearance.theme.value).toBe("dark")
    expect(appearance.colorScheme.value).toBe("rose")
    expect(appearance.uiFont.value).toBe("system")
    expect(appearance.contentFont.value).toBe("mono")
    expect(appearance.railAppearance.value).toBe("solid")
    expect(appearance.uiFontWeight.value).toBe("regular")
    for (const key of MANAGED_KEYS) {
      expect(localStorage.getItem(key)).toBeNull()
    }
  })
})
