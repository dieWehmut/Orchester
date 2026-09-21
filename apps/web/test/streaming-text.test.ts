import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import { createEmptyRunView } from "@orchester/ereignis"
import RunTimeline from "../src/components/run/RunTimeline.vue"
import { arrivalState } from "../src/components/run/streaming-text"

/**
 * Word-arrival streaming, task U3-07 of the implementation plan.
 *
 * Two rules make streaming text readable rather than a flicker: a turn still
 * arriving says so, and its arrival may not reflow what is around it. The
 * second rule is the one that is usually missed - a growing text node measures
 * its ancestors on every token, and a virtualised transcript re-measures every
 * row as a result. The transcript therefore marks the arriving turn and
 * contains its layout, so the rows around it keep the boxes they had.
 */
describe("arrivalState", () => {
  it("marks a turn still arriving and leaves a finished one alone", () => {
    expect(arrivalState({ final: false })).toBe("streaming")
    expect(arrivalState({ final: true })).toBe("settled")
  })
})

describe("streaming turns in the transcript", () => {
  const arriving = {
    ...createEmptyRunView(),
    timeline: [
      {
        type: "message" as const,
        key: "message-1",
        sequence: 1,
        occurredAt: "2026-09-20T06:00:00Z",
        turnId: null,
        role: "assistant" as const,
        text: "still arriving",
        final: false,
      },
      {
        type: "message" as const,
        key: "message-2",
        sequence: 2,
        occurredAt: "2026-09-20T06:01:00Z",
        turnId: null,
        role: "assistant" as const,
        text: "already here",
        final: true,
      },
    ],
  }

  it("reports which turn is still arriving", () => {
    const wrapper = mount(RunTimeline, { props: { view: arriving } })

    const rows = wrapper.findAll("[data-virtualized-turn]")
    expect(rows[0]!.attributes("data-arrival-state")).toBe("streaming")
    expect(rows[1]!.attributes("data-arrival-state")).toBe("settled")
  })

  it("contains the arriving turn so its growth cannot reflow its ancestors", () => {
    const wrapper = mount(RunTimeline, { props: { view: arriving } })

    // The style is the contract here: `contain` is what keeps a token from
    // measuring the transcript and re-running the virtual window on every word.
    const streaming = wrapper.get('[data-arrival-state="streaming"]')
    expect(streaming.attributes("style")).toContain("contain")
    expect(streaming.attributes("style")).toContain("content-visibility")
  })

  it("does not announce the arriving tokens", () => {
    // Section 7: streaming tokens are not announced; the turn is announced once
    // on completion. A live region on the arriving text would read every word.
    const wrapper = mount(RunTimeline, { props: { view: arriving } })
    const streaming = wrapper.get('[data-arrival-state="streaming"]')
    expect(streaming.attributes("aria-live")).toBeUndefined()
    expect(streaming.attributes("role")).toBeUndefined()
  })
})
