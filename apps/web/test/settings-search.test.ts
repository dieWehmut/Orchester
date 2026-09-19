import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import { filterSettingsSections, SETTINGS_SECTIONS } from "../src/components/settings/settings-search"
import SettingsView from "../src/views/SettingsView.vue"

/**
 * Search across the settings panels exists so a reader who knows what they want
 * does not have to learn the panel order. It matches on what the section is
 * called and on what it covers, and it says plainly when nothing matches rather
 * than showing an empty list.
 */
describe("settings search", () => {
  const sections = [
    { id: "appearance", label: "Appearance", keywords: ["theme", "colour", "font"] },
    { id: "providers", label: "Providers", keywords: ["model", "api key"] },
  ]

  it("returns every section for an empty query", () => {
    expect(filterSettingsSections(sections, "")).toEqual(sections)
    expect(filterSettingsSections(sections, "   ")).toEqual(sections)
  })

  it("matches on the section name regardless of case", () => {
    const [appearance] = filterSettingsSections(sections, "appear")
    expect(appearance?.id).toBe("appearance")
  })

  it("matches on what the section covers, not only its name", () => {
    const [providers] = filterSettingsSections(sections, "api key")
    expect(providers?.id).toBe("providers")
  })

  it("returns nothing rather than everything when a query matches nothing", () => {
    expect(filterSettingsSections(sections, "kubernetes")).toEqual([])
  })

  it("ships a searchable entry for every section the settings surface renders", () => {
    const ids = SETTINGS_SECTIONS.map((section) => section.id)
    expect(ids).toEqual([
      "general",
      "appearance",
      "notifications",
      "import",
      "profile",
      "providers",
      "about",
    ])
    expect(SETTINGS_SECTIONS.every((section) => section.keywords.length > 0)).toBe(true)
  })

  it("filters the settings navigation from the search field", async () => {
    const wrapper = mount(SettingsView)
    const search = wrapper.find("[data-settings-search]")

    expect(search.exists()).toBe(true)

    await search.find("input").setValue("provider")
    expect(
      wrapper.findAll("[data-settings-nav-link]").map((node) => node.attributes("data-settings-nav-link")),
    ).toEqual(["providers"])

    await search.find("input").setValue("")
    expect(wrapper.findAll("[data-settings-nav-link]")).toHaveLength(SETTINGS_SECTIONS.length)
  })

  it("says when a search matched nothing instead of showing an empty nav", async () => {
    const wrapper = mount(SettingsView)
    const search = wrapper.find("[data-settings-search]")

    await search.find("input").setValue("kubernetes")

    expect(wrapper.findAll("[data-settings-nav-link]")).toHaveLength(0)
    expect(wrapper.find("[data-settings-search-empty]").exists()).toBe(true)
  })
})
