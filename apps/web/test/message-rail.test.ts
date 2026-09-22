import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import { createEmptyRunView } from "@orchester/ereignis"
import MessageRail from "../src/components/run/MessageRail.vue"
import { railMarks } from "../src/components/run/message-rail"

/**
 * The message navigation rail, task U3-04 of the implementation plan.
 *
 * A governed run can be hundreds of turns long, and the one thing a reader
 * wants is to get back to a specific user turn. The rail lists those turns,
 * expands a label on hover, and scrubs the transcript while dragged.
 */

function userTurn(n: number) {
  return {
    type: "message" as const,
    key: `message-${n}`,
    sequence: n * 2,
    occurredAt: "2026-09-20T06:00:00Z",
    turnId: null,
    role: "user" as const,
    text: `question ${n}`,
    final: true,
  }
}

function assistantTurn(n: number) {
  return { ...userTurn(n), role: "assistant" as const, text: `answer ${n}` }
}

const view = {
  ...createEmptyRunView(),
  timeline: [userTurn(1), assistantTurn(1), userTurn(2), assistantTurn(2), userTurn(3)],
}

describe("railMarks", () => {
  it("lists the user turns and no others", () => {
    // The rail answers "where did I ask that?", so an assistant answer is not a
    // destination. The transcript already shows it under its question.
    const marks = railMarks(view.timeline)
    expect(marks.map((mark) => mark.index)).toEqual([0, 2, 4])
    expect(marks.map((mark) => mark.label)).toEqual(["question 1", "question 2", "question 3"])
  })

  it("truncates a long question rather than growing the rail", () => {
    const long = {
      ...createEmptyRunView(),
      timeline: [{ ...userTurn(1), text: "x".repeat(200) }],
    }
    expect(railMarks(long.timeline)[0]!.label.length).toBeLessThanOrEqual(80)
  })
})

describe("MessageRail", () => {
  it("renders one mark per user turn and no marks for a run without any", () => {
    const wrapper = mount(MessageRail, { props: { view } })
    expect(wrapper.findAll("[data-rail-mark]")).toHaveLength(3)

    const empty = mount(MessageRail, {
      props: { view: { ...createEmptyRunView(), timeline: [assistantTurn(1)] } },
    })
    expect(empty.find("[data-message-rail]").exists()).toBe(false)
  })

  it("jumps to the turn when a mark is chosen", async () => {
    const wrapper = mount(MessageRail, { props: { view } })
    await wrapper.findAll("[data-rail-mark]")[1]!.trigger("click")
    expect(wrapper.emitted("select")?.[0]).toEqual([2])
  })

  it("scrubs the transcript while the rail is dragged", async () => {
    const wrapper = mount(MessageRail, { props: { view } })
    const rail = wrapper.get("[data-message-rail]")
    const marks = wrapper.findAll("[data-rail-mark]")

    await rail.trigger("pointerdown")
    expect(rail.attributes("data-scrubbing")).toBe("true")

    // The pointer reports which mark it is over; the rail turns that into the
    // jump, because a drag that only says it is scrubbing moves nothing.
    await marks[1]!.trigger("pointermove")
    await marks[0]!.trigger("pointermove")
    // Re-reporting the mark already chosen would re-scroll the transcript on
    // every pixel of the drag.
    await marks[0]!.trigger("pointermove")

    expect(wrapper.emitted("select")).toEqual([[2], [0]])

    await rail.trigger("pointerup")
    expect(rail.attributes("data-scrubbing")).toBe("false")
  })

  it("keeps the label of the mark it is dragging to on screen", async () => {
    const wrapper = mount(MessageRail, { props: { view } })
    const rail = wrapper.get("[data-message-rail]")
    const marks = wrapper.findAll("[data-rail-mark]")

    await rail.trigger("pointerdown")
    await marks[1]!.trigger("pointermove")

    // Aiming needs the words: a reader dragging the rail is choosing a turn by
    // its question, which is not on screen anywhere else.
    expect(marks[1]!.get("[data-rail-label]").text()).toBe("question 2")

    await rail.trigger("pointercancel")

    expect(marks[1]!.find("[data-rail-label]").exists()).toBe(false)
    expect(rail.attributes("data-scrubbing")).toBe("false")
  })

  it("marks the turn the rail has jumped to", async () => {
    const wrapper = mount(MessageRail, { props: { view } })

    await wrapper.findAll("[data-rail-mark]")[1]!.trigger("click")

    expect(wrapper.findAll("[data-rail-active]")).toHaveLength(1)
    expect(wrapper.get("[data-rail-active]").attributes("data-rail-index")).toBe("2")
  })

  it("expands a label on hover rather than showing every label at once", async () => {
    const wrapper = mount(MessageRail, { props: { view } })
    const mark = wrapper.findAll("[data-rail-mark]")[0]!

    expect(mark.find("[data-rail-label]").exists()).toBe(false)
    await mark.trigger("pointerenter")
    expect(mark.get("[data-rail-label]").text()).toBe("question 1")
  })
})
