import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import { createEmptyRunView } from "@orchester/ereignis"
import RunTimeline from "../src/components/run/RunTimeline.vue"

/**
 * A governed run emits far more tool calls than messages, so the transcript
 * cannot print each one as a wall of text. A card names the invocation, says
 * what state it ended in, and keeps the detail behind a disclosure the reader
 * can open when the call is the thing they are investigating.
 */
describe("RunTimeline tool cards", () => {
  const view = {
    ...createEmptyRunView(),
    timeline: [
      {
        type: "tool" as const,
        key: "tool-1",
        sequence: 4,
        occurredAt: "2026-09-19T03:00:00Z",
        turnId: null,
        callId: "call-1" as never,
        name: "read_file",
        state: "succeeded" as const,
        detail: "src/main.rs\nfn main() {}",
      },
    ],
  }

  it("renders a tool call as a card keyed by its call id", () => {
    const wrapper = mount(RunTimeline, { props: { view } })

    const card = wrapper.get("[data-tool-card]")
    expect(card.attributes("data-tool-call-id")).toBe("call-1")
    expect(card.attributes("data-tool-state")).toBe("succeeded")
    expect(card.text()).toContain("read_file")
  })

  it("keeps the tool detail collapsed until the reader asks for it", async () => {
    const wrapper = mount(RunTimeline, { props: { view } })

    expect(wrapper.find("[data-tool-detail]").exists()).toBe(false)

    const toggle = wrapper.get("[data-tool-toggle]")
    expect(toggle.attributes("aria-expanded")).toBe("false")

    await toggle.trigger("click")

    expect(toggle.attributes("aria-expanded")).toBe("true")
    expect(wrapper.get("[data-tool-detail]").text()).toContain("fn main() {}")
  })

  it("summarises a failed call rather than hiding it", () => {
    const failed = {
      ...view,
      timeline: [
        { ...view.timeline[0]!, state: "failed" as const, detail: "permission denied" },
      ],
    }
    const wrapper = mount(RunTimeline, { props: { view: failed } })

    const card = wrapper.get("[data-tool-card]")
    expect(card.attributes("data-tool-state")).toBe("failed")
    expect(card.text().toLowerCase()).toContain("failed")
  })

  it("omits the disclosure when the call has no detail", () => {
    const bare = {
      ...view,
      timeline: [{ ...view.timeline[0]!, detail: null }],
    }
    const wrapper = mount(RunTimeline, { props: { view: bare } })

    expect(wrapper.find("[data-tool-toggle]").exists()).toBe(false)
  })
})
