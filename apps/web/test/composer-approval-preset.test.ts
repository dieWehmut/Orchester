import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import ApprovalPresetControl from "../src/components/run/ApprovalPresetControl.vue"

/**
 * Permissions are the one composer control that can hand an agent the machine,
 * so the presets have to be named, the dangerous one has to be confirmed, and
 * the chosen scope has to be visible from the DOM rather than from a tooltip.
 */
describe("ApprovalPresetControl", () => {
  it("names the three presets and reports the current one", () => {
    const wrapper = mount(ApprovalPresetControl, { props: { modelValue: "ask" } })

    expect(wrapper.get("[data-approval-preset]").attributes("data-approval-preset-state")).toBe("ask")
    expect(wrapper.get("[data-approval-preset-label]").text()).toBe("Ask")

    const trigger = wrapper.get('[data-approval-preset] [aria-haspopup="menu"]')
    expect(trigger.attributes("aria-haspopup")).toBe("menu")
    expect(trigger.text()).toContain("Ask")
  })

  it("offers Ask, Governed and Full access in the menu", async () => {
    const wrapper = mount(ApprovalPresetControl, { props: { modelValue: "ask" } })

    await wrapper.get("[data-approval-preset-trigger]").trigger("click")

    const choices = wrapper.findAll('[role="menuitemradio"]')
    const items = choices.map((item) => item.get('strong').text())
    expect(items).toEqual(["Ask", "Governed", "Full access"])
    expect(choices[0]!.attributes('aria-checked')).toBe('true')
    expect(choices[1]!.get('small').text()).toContain('approval policy')
  })

  it("switches straight to Governed without a confirmation", async () => {
    const wrapper = mount(ApprovalPresetControl, { props: { modelValue: "ask" } })

    await wrapper.get("[data-approval-preset-trigger]").trigger("click")
    await wrapper.findAll('[role="menuitemradio"]')[1]!.trigger("click")

    expect(wrapper.emitted("update:modelValue")).toEqual([["governed"]])
    expect(wrapper.find("[data-approval-preset-confirm]").exists()).toBe(false)
  })

  it("requires an explicit confirmation before Full access", async () => {
    const wrapper = mount(ApprovalPresetControl, { props: { modelValue: "ask" } })

    await wrapper.get("[data-approval-preset-trigger]").trigger("click")
    await wrapper.findAll('[role="menuitemradio"]')[2]!.trigger("click")

    // The risky combination is not applied by picking it.
    expect(wrapper.emitted("update:modelValue")).toBeUndefined()
    expect(wrapper.get("[data-approval-preset-confirm]")).toBeTruthy()

    await wrapper.get('[data-approval-preset-confirm="accept"]').trigger("click")
    expect(wrapper.emitted("update:modelValue")).toEqual([["full-access"]])
  })

  it("cancels the risky confirmation without changing the preset", async () => {
    const wrapper = mount(ApprovalPresetControl, { props: { modelValue: "ask" } })

    await wrapper.get("[data-approval-preset-trigger]").trigger("click")
    await wrapper.findAll('[role="menuitemradio"]')[2]!.trigger("click")
    await wrapper.get('[data-approval-preset-confirm="cancel"]').trigger("click")

    expect(wrapper.emitted("update:modelValue")).toBeUndefined()
    expect(wrapper.find("[data-approval-preset-confirm]").exists()).toBe(false)
  })

  it("paints the composer danger intent while Full access is selected", () => {
    const wrapper = mount(ApprovalPresetControl, { props: { modelValue: "full-access" } })

    const root = wrapper.get("[data-approval-preset]")
    expect(root.attributes("data-approval-preset-state")).toBe("full-access")
    expect(wrapper.get("[data-approval-preset-label]").text()).toBe("Full access")
    expect(root.attributes("data-approval-preset-danger")).toBe("true")
  })
})
