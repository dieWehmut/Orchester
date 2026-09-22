import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import { createI18n } from "../src/i18n"
import ReasoningDisclosure from "../src/components/run/ReasoningDisclosure.vue"

/**
 * Reasoning is the agent thinking out loud: useful when a result looks wrong,
 * noise when a run is healthy. It stays collapsed until the reader asks, and the
 * summary says how much is behind the disclosure so opening it is a choice made
 * with information rather than a guess.
 */
describe("ReasoningDisclosure", () => {
  it("keeps the reasoning collapsed behind a labelled disclosure", () => {
    const wrapper = mount(ReasoningDisclosure, {
      props: { text: "Check the boundary before editing." },
    })

    const toggle = wrapper.get("[data-reasoning-toggle]")
    expect(toggle.attributes("aria-expanded")).toBe("false")
    expect(toggle.text()).toContain("Reasoning")
    expect(wrapper.find("[data-reasoning-body]").exists()).toBe(false)
  })

  it("reveals the reasoning when opened and hides it again", async () => {
    const wrapper = mount(ReasoningDisclosure, {
      props: { text: "Check the boundary before editing." },
    })

    await wrapper.get("[data-reasoning-toggle]").trigger("click")

    expect(wrapper.get("[data-reasoning-toggle]").attributes("aria-expanded")).toBe("true")
    expect(wrapper.get("[data-reasoning-body]").text()).toContain("Check the boundary")

    await wrapper.get("[data-reasoning-toggle]").trigger("click")

    expect(wrapper.find("[data-reasoning-body]").exists()).toBe(false)
  })

  it("reports how much reasoning is behind the disclosure", () => {
    const wrapper = mount(ReasoningDisclosure, {
      props: { text: "x".repeat(420) },
    })

    expect(wrapper.get("[data-reasoning-summary]").text()).toContain("420")
  })

  it("stays out of the transcript when there is no reasoning", () => {
    const wrapper = mount(ReasoningDisclosure, { props: { text: "   " } })

    expect(wrapper.find("[data-reasoning-disclosure]").exists()).toBe(false)
  })

  it("counts the reasoning in the reader's own language", () => {
    // The count is copy, not a number with a word glued to it: this row is the
    // transcript's, so it goes through the catalogue like the rest of it.
    const wrapper = mount(ReasoningDisclosure, {
      props: { text: "x".repeat(12) },
      global: { plugins: [createI18n("zh-CN")] },
    })

    const summary = wrapper.get("[data-reasoning-summary]").text()

    expect(summary).toContain("12")
    expect(summary).not.toContain("characters")
  })
})

