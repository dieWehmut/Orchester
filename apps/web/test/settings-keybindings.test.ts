import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"
import { nextTick } from "vue"

import SettingsView from "../src/views/SettingsView.vue"
import { shortcutRegistry } from "../src/shortcuts"

/**
 * The keybindings panel.
 *
 * The editor reads the shared registry, so the panel has to hand it the same
 * registry the shell dispatches from. A second registry would produce a panel
 * that edits a list nothing answers to, which is the drift the registry was
 * built to prevent.
 */
describe("settings keybindings panel", () => {
  it("reaches the keybindings section from the navigation", async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-settings-nav-link="keybindings"]').trigger("click")

    expect(wrapper.get('[data-settings-section="keybindings"]').attributes("aria-selected")).toBe("true")
  })

  it("renders the editor against the registry the shell dispatches from", async () => {
    const wrapper = mount(SettingsView)
    const panel = wrapper.get('[data-settings-section="keybindings"]')

    expect(panel.find("[data-shortcut-editor]").exists()).toBe(true)

    // The registry is the app's own: registering into it must appear here.
    shortcutRegistry.register({
      id: "test.appears",
      labelKey: "shortcuts.labels.settingsOpen",
      groupKey: "shortcuts.groups.layout",
      keys: ["Mod", "Shift", "Y"],
    })
    await nextTick()

    expect(panel.find('[data-shortcut-row="test.appears"]').exists()).toBe(true)
  })
})
