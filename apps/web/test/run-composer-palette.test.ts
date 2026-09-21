import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import RunComposer from "../src/components/run/RunComposer.vue"
import { COMPOSER_COMMANDS } from "../src/components/run/composer-commands"

/**
 * The palette is opened from the composer, so the composer owns the keystroke
 * that opens it and the escape that closes it. The command list is shared with
 * the TUI's own /help vocabulary rather than invented here.
 */
describe("RunComposer command palette", () => {
  it("opens the palette as soon as the draft starts with a slash", async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })

    expect(wrapper.find("[data-command-palette]").exists()).toBe(false)

    await wrapper.get("textarea").setValue("/")

    expect(wrapper.get("[data-command-palette]")).toBeTruthy()
    expect(wrapper.findAll("[data-command-item]").length).toBe(COMPOSER_COMMANDS.length)
  })

  it("narrows the palette to what the reader typed", async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })

    await wrapper.get("textarea").setValue("/mod")

    const items = wrapper.findAll("[data-command-item]")
    expect(items).toHaveLength(1)
    expect(items[0]!.text()).toContain("/model")
  })

  it("closes the palette on Escape and keeps the draft", async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })
    await wrapper.get("textarea").setValue("/mod")

    await wrapper.get("[data-command-palette]").trigger("keydown", { key: "Escape" })

    expect(wrapper.find("[data-command-palette]").exists()).toBe(false)
    expect(wrapper.get("textarea").element.value).toBe("/mod")
  })

  it("closes the palette once the draft is no longer a command", async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })
    await wrapper.get("textarea").setValue("/model")

    await wrapper.get("textarea").setValue("explain the boundary")

    expect(wrapper.find("[data-command-palette]").exists()).toBe(false)
  })

  it("shows the palette above the prompt, not over the action row", async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })
    await wrapper.get("textarea").setValue("/")

    const palette = wrapper.get("[data-command-palette]").element
    const textarea = wrapper.get("textarea").element
    expect(palette.compareDocumentPosition(textarea) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })
})

