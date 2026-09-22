import { mount } from "@vue/test-utils"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { resetAppearanceForTests } from "@orchester/design"

import SettingsView from "../src/views/SettingsView.vue"
import { THEME_COLOURS } from "../src/components/settings/theme-colours"

/**
 * The two theme cards the reference draws under the preview.
 *
 * The reference does not offer one accent for the product: it offers a light
 * theme and a dark theme, each with the hue it is set in, its own background
 * and its own foreground, and each with the actions that read or write that
 * one theme. A surface that reported the active theme's colours in both cards
 * would be showing the reader the same column twice.
 */

const originalMatchMedia = window.matchMedia

/**
 * How jsdom echoes a hex colour back in an inline `style`.
 *
 * The assertion is about which colour painted the swatch, not about how the
 * browser chose to spell it, so the expected value is normalised the same way
 * the DOM normalises it.
 */
function asRgb(hex: string): string {
  const value = hex.replace('#', '')
  const [red, green, blue] = [0, 2, 4].map((offset) =>
    Number.parseInt(value.slice(offset, offset + 2), 16),
  )
  return `rgb(${red}, ${green}, ${blue})`
}

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
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe("appearance theme cards", () => {
  it("gives each theme its own card and its own hue", async () => {
    const wrapper = mount(SettingsView)

    const light = wrapper.get('[data-appearance-theme="light"]')
    const dark = wrapper.get('[data-appearance-theme="dark"]')

    expect(light.findAll('[data-appearance-field="scheme"] select option')).toHaveLength(4)
    expect(dark.findAll('[data-appearance-field="scheme"] select option')).toHaveLength(4)

    // The system asks for dark here, so the light card's hue must not touch the
    // attribute the stylesheet matches on.
    await light.get('[data-appearance-field="scheme"] select').setValue("teal")

    expect(localStorage.getItem("orchester:color-schemes")).toBe(
      JSON.stringify({ light: "teal", dark: "rose" }),
    )
    expect(document.documentElement.getAttribute("data-color-scheme")).toBe("rose")

    await wrapper.get('[data-theme-option="light"]').trigger("click")
    await light.get('[data-appearance-field="scheme"] select').setValue("teal")

    expect(document.documentElement.getAttribute("data-color-scheme")).toBe("teal")
  })

  it("prints each theme's own colours rather than the active theme's", () => {
    const wrapper = mount(SettingsView)

    // The theme in force is dark; the light card still reports the light
    // values, because it is describing the theme it names.
    expect(
      wrapper.get('[data-appearance-theme="light"] [data-appearance-field="background"] [data-color-hex]').text(),
    ).toBe(THEME_COLOURS.light.background)
    expect(
      wrapper.get('[data-appearance-theme="dark"] [data-appearance-field="background"] [data-color-hex]').text(),
    ).toBe(THEME_COLOURS.dark.background)

    for (const field of ["accent", "background", "foreground"]) {
      expect(
        wrapper
          .get(`[data-appearance-theme="dark"] [data-appearance-field="${field}"]`)
          .find("[data-color-readout]")
          .exists(),
      ).toBe(true)
    }
  })

  it("paints the accent readout from the hue that theme is set in", async () => {
    const wrapper = mount(SettingsView)

    await wrapper
      .get('[data-appearance-theme="light"] [data-appearance-field="scheme"] select')
      .setValue("teal")
    await wrapper
      .get('[data-appearance-theme="dark"] [data-appearance-field="scheme"] select')
      .setValue("teal")

    // The same scheme on two themes is two colours: the light accent is
    // darkened until it clears AA, and a single swatch could not say both.
    const lightAccent = wrapper
      .get('[data-appearance-theme="light"] [data-appearance-field="accent"] [data-color-hex]')
      .text()
    const darkAccent = wrapper
      .get('[data-appearance-theme="dark"] [data-appearance-field="accent"] [data-color-hex]')
      .text()

    expect(lightAccent).not.toBe(darkAccent)
    expect(
      wrapper.get('[data-appearance-theme="light"] [data-theme-swatch]').attributes("style"),
    ).toContain(asRgb(lightAccent))
  })

  it("carries the reference's own actions on each theme card", async () => {
    const writes: string[] = []
    const clicks = vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(() => undefined)
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (value: string) => {
          writes.push(value)
          return Promise.resolve()
        },
      },
    })

    const wrapper = mount(SettingsView)

    await wrapper
      .get('[data-appearance-theme="light"] [data-action="import-theme"]')
      .trigger("click")
    expect(clicks).toHaveBeenCalled()
    // One file input, however many rows offer to read it: three pickers would
    // be three places for the same parser to drift.
    expect(wrapper.findAll('input[type="file"]')).toHaveLength(1)

    await wrapper.get('[data-appearance-theme="light"] [data-action="copy-theme"]').trigger("click")

    const payload = JSON.parse(writes[0] ?? "{}") as Record<string, string>
    expect(payload.theme).toBe("light")
    expect(payload.colorScheme).toBe("rose")
    expect(payload.background).toBe(THEME_COLOURS.light.background)
    expect(payload.foreground).toBe(THEME_COLOURS.light.foreground)
    expect(payload.accent).toMatch(/^#[0-9A-F]{6}$/)
  })
})