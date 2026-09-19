import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import RunComposer from "../src/components/run/RunComposer.vue"

/**
 * The composer is the one surface that is always on screen, so its state has to
 * be readable from the DOM rather than inferred from which button happens to be
 * disabled. The design spec names the five states; a run in flight must never
 * look the same as one waiting for input.
 */
describe("RunComposer state", () => {
  it("reports the idle state when nothing is running", () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })
    expect(wrapper.get("[data-run-composer]").attributes("data-composer-state")).toBe("idle")
  })

  it("reports submitting separately from running", () => {
    const submitting = mount(RunComposer, { props: { modelValue: "go", lifecycle: "submitting" } })
    expect(submitting.get("[data-run-composer]").attributes("data-composer-state")).toBe("submitting")

    const running = mount(RunComposer, { props: { modelValue: "go", lifecycle: "running" } })
    expect(running.get("[data-run-composer]").attributes("data-composer-state")).toBe("running")
  })

  it("reports cancelling while a run is being stopped", () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "go", lifecycle: "cancelling" } })
    expect(wrapper.get("[data-run-composer]").attributes("data-composer-state")).toBe("cancelling")
  })

  it("derives busy from the lifecycle so a run in flight cannot look idle", async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "go", lifecycle: "running" } })

    expect(wrapper.get("textarea").attributes("disabled")).toBeDefined()
    expect(wrapper.find('button[type="submit"]').exists()).toBe(false)
    expect(wrapper.get('[data-composer-action="cancel"]')).toBeTruthy()

    // A settled lifecycle frees the input again even while `busy` stays unset.
    await wrapper.setProps({ lifecycle: "completed" })
    expect(wrapper.get("textarea").attributes("disabled")).toBeUndefined()
    expect(wrapper.get('[data-composer-action="submit"]')).toBeTruthy()
  })

  it("reports a drag in progress regardless of what is running", () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "", dragActive: true } })
    expect(wrapper.get("[data-run-composer]").attributes("data-composer-state")).toBe("dragging")
  })

  it("grows the prompt from one row and never past twelve", () => {
    const wrapper = mount(RunComposer, { props: { modelValue: "" } })
    const textarea = wrapper.get("textarea")

    expect(textarea.attributes("rows")).toBe("1")

    // Twelve explicit newlines would be thirteen lines; the cap holds.
    wrapper.setProps({ modelValue: "\n".repeat(20) })
    return wrapper.vm.$nextTick().then(() => {
      expect(Number(textarea.attributes("rows"))).toBeLessThanOrEqual(12)
    })
  })
})
