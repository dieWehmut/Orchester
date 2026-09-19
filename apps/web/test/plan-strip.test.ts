import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import PlanStrip from "../src/components/run/PlanStrip.vue"

/**
 * The plan strip shows the agent's plan as progress, not as a list. Its state
 * has to be readable at a glance: a blocked plan is what surfaces "needs input",
 * so it cannot look like a plan that is merely working.
 */
describe("PlanStrip", () => {
  it("stays out of the way when the agent has no plan", () => {
    const wrapper = mount(PlanStrip, { props: { todos: [] } })

    expect(wrapper.find("[data-plan-strip]").exists()).toBe(false)
  })

  it("reports an active plan with its segmented progress", () => {
    const wrapper = mount(PlanStrip, {
      props: {
        todos: [
          { text: "Read the runtime", completed: true },
          { text: "Patch the boundary", completed: false },
          { text: "Re-run the gate", completed: false },
        ],
      },
    })

    const strip = wrapper.get("[data-plan-strip]")
    expect(strip.attributes("data-plan-state")).toBe("active")
    expect(strip.get("[data-plan-current]").text()).toContain("Patch the boundary")

    const segments = strip.findAll("[data-plan-segment]")
    expect(segments).toHaveLength(3)
    expect(segments[0]!.attributes("data-plan-segment-state")).toBe("done")
    expect(segments[1]!.attributes("data-plan-segment-state")).toBe("current")
    expect(segments[2]!.attributes("data-plan-segment-state")).toBe("pending")
    expect(strip.get("[data-plan-progress]").attributes("aria-valuenow")).toBe("1")
    expect(strip.get("[data-plan-progress]").attributes("aria-valuemax")).toBe("3")
  })

  it("reports a finished plan as done", () => {
    const wrapper = mount(PlanStrip, {
      props: {
        todos: [
          { text: "Read the runtime", completed: true },
          { text: "Re-run the gate", completed: true },
        ],
      },
    })

    expect(wrapper.get("[data-plan-strip]").attributes("data-plan-state")).toBe("done")
    expect(wrapper.find("[data-plan-current]").exists()).toBe(false)
  })

  it("surfaces a blocked plan as the needs-input treatment", () => {
    const wrapper = mount(PlanStrip, {
      props: {
        todos: [{ text: "Wait for the approval", completed: false }],
        blocked: true,
      },
    })

    const strip = wrapper.get("[data-plan-strip]")
    expect(strip.attributes("data-plan-state")).toBe("blocked")
    expect(strip.attributes("data-plan-needs-input")).toBe("true")
    expect(strip.get("[data-plan-current]").text()).toContain("Wait for the approval")
  })

  it("expands to the full list and the validator results", async () => {
    const wrapper = mount(PlanStrip, {
      props: {
        todos: [
          { text: "Read the runtime", completed: true },
          { text: "Re-run the gate", completed: false },
        ],
        validation: {
          ok: false,
          summary: "2 of 5 checks failed",
        },
      },
    })

    expect(wrapper.find("[data-plan-details]").exists()).toBe(false)

    await wrapper.get("[data-plan-toggle]").trigger("click")

    const details = wrapper.get("[data-plan-details]")
    expect(details.findAll("[data-plan-item]")).toHaveLength(2)
    expect(details.get("[data-plan-validation]").text()).toContain("2 of 5 checks failed")
  })
})
