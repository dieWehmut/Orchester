import { flushPromises, mount } from "@vue/test-utils"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import SettingsView from "../src/views/SettingsView.vue"
import { resetAppearanceForTests } from "@orchester/design"

const originalMatchMedia = window.matchMedia

/** jsdom answers every media query false and never emits a change. */
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

function mountSettings() {
  return mount(SettingsView)
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
  vi.unstubAllGlobals()
})

/**
 * The buttons are only half a feature if the file they write cannot be read
 * back. This drives both through the view rather than calling the profile
 * helpers directly, because the wiring between them is the part that can break
 * without either half noticing.
 */
describe("appearance profile files", () => {
  it("writes a profile that a later session can read back", async () => {
    const written: string[] = []
    const downloads: string[] = []
    vi.stubGlobal("URL", {
      createObjectURL: vi.fn(() => "blob:profile"),
      revokeObjectURL: vi.fn(),
    })
    // jsdom cannot navigate, and logs a "Not implemented" error for every
    // anchor click. Swallowing the click keeps the console honest without
    // changing how the export actually runs.
    const clickSpy = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(function (this: HTMLAnchorElement) {
        downloads.push(this.download)
      })
    const OriginalBlob = globalThis.Blob
    class CapturingBlob extends OriginalBlob {
      constructor(parts: BlobPart[], options?: BlobPropertyBag) {
        super(parts, options)
        written.push(String(parts[0]))
      }
    }
    vi.stubGlobal("Blob", CapturingBlob)

    const exporter = mountSettings()
    await exporter.get('[data-appearance-field="ui-font"] select').setValue("serif")
    await exporter.get('[data-settings-actions] [data-action="export"]').trigger("click")
    await flushPromises()

    const payload = written[0] ?? ""
    expect(payload.length).toBeGreaterThan(0)
    expect(downloads).toEqual(["orchester-appearance.json"])
    clickSpy.mockRestore()

    // A fresh session: nothing stored, nothing on the document.
    resetAppearanceForTests()
    for (const attribute of document.documentElement.getAttributeNames()) {
      if (attribute.startsWith("data-")) document.documentElement.removeAttribute(attribute)
    }

    const importer = mountSettings()
    const file = new File([payload], "orchester-appearance.json", {
      type: "application/json",
    })
    const input = importer.get('input[type="file"]')
    Object.defineProperty(input.element, "files", { value: [file], configurable: true })
    await input.trigger("change")
    await flushPromises()

    expect(document.documentElement.getAttribute("data-ui-font")).toBe("serif")
    expect(localStorage.getItem("orchester:ui-font")).toBe("serif")
  })
})
