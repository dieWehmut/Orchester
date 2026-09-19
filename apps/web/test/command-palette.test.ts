import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import CommandPalette from "../src/components/run/CommandPalette.vue"

/**
 * The palette is opened by a keystroke and closed by one, so it has to be
 * complete without a pointer: the reader never sees a bare list. It names what
 * each command does, says when nothing matches, and says when it is still
 * loading rather than pretending the list is complete.
 */
describe("CommandPalette", () => {
  const commands = [
    { id: "agent", name: "/agent", description: "choose a delegate" },
    { id: "model", name: "/model", description: "choose a model or provider" },
    { id: "theme", name: "/theme", description: "preview terminal colors" },
  ]

  it("lists every command with what it does", () => {
    const wrapper = mount(CommandPalette, { props: { open: true, commands } })

    const items = wrapper.findAll("[data-command-item]")
    expect(items).toHaveLength(3)
    expect(items[0]!.text()).toContain("/agent")
    expect(items[0]!.text()).toContain("choose a delegate")
    expect(wrapper.get("[data-command-palette]").attributes("role")).toBe("listbox")
  })

  it("filters as the reader types and says so when nothing matches", async () => {
    const wrapper = mount(CommandPalette, { props: { open: true, commands, query: "/mod" } })

    expect(wrapper.findAll("[data-command-item]")).toHaveLength(1)
    expect(wrapper.get("[data-command-item]").text()).toContain("/model")

    await wrapper.setProps({ query: "/nonsense" })

    expect(wrapper.findAll("[data-command-item]")).toHaveLength(0)
    expect(wrapper.get("[data-command-empty]").text().length).toBeGreaterThan(0)
  })

  it("shows a loading state instead of an empty one while it is loading", async () => {
    const wrapper = mount(CommandPalette, { props: { open: true, commands: [], loading: true } })

    expect(wrapper.get("[data-command-loading]")).toBeTruthy()
    expect(wrapper.find("[data-command-empty]").exists()).toBe(false)
  })

  it("is absent while closed and emits the chosen command", async () => {
    const closed = mount(CommandPalette, { props: { open: false, commands } })
    expect(closed.find("[data-command-palette]").exists()).toBe(false)

    const wrapper = mount(CommandPalette, { props: { open: true, commands } })
    await wrapper.findAll("[data-command-item]")[1]!.trigger("click")

    expect(wrapper.emitted("select")).toEqual([["model"]])
  })

  it("moves the highlight with the arrow keys and opens on Enter", async () => {
    const wrapper = mount(CommandPalette, { props: { open: true, commands } })
    const palette = wrapper.get("[data-command-palette]")

    expect(wrapper.get('[data-command-active="true"]').text()).toContain("/agent")

    await palette.trigger("keydown", { key: "ArrowDown" })
    expect(wrapper.get('[data-command-active="true"]').text()).toContain("/model")

    await palette.trigger("keydown", { key: "Enter" })
    expect(wrapper.emitted("select")).toEqual([["model"]])

    await palette.trigger("keydown", { key: "Escape" })
    expect(wrapper.emitted("close")).toHaveLength(1)
  })
})

