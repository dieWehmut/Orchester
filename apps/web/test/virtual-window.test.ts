import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"

import { createEmptyRunView } from "@orchester/ereignis"
import RunTimeline from "../src/components/run/RunTimeline.vue"
import { virtualWindow } from "../src/components/run/virtual-window"

/**
 * The transcript virtualises turns, task U3-03 of the implementation plan.
 *
 * A governed run can produce thousands of turns and the reader is usually
 * looking at the last few, so mounting all of them costs memory and layout for
 * rows nobody will see. The window is computed from heights that were actually
 * measured; an unmeasured row is not guessed at, because a guessed height moves
 * the reader's place in the transcript under them.
 */
describe("transcript virtual window", () => {
  const heights = [100, 100, 100, 100, 100, 100, 100, 100, 100, 100]

  it("mounts the rows around the viewport and the overscan either side", () => {
    // Viewport covers rows 3 and 4 (300..500); one row of overscan each side.
    const window = virtualWindow({
      heights,
      scrollTop: 320,
      viewportHeight: 180,
      overscan: 1,
    })

    expect(window.start).toBe(2)
    expect(window.end).toBe(6)
    expect(window.totalHeight).toBe(1000)
    expect(window.offsetTop).toBe(200)
  })

  it("starts at the first row and ends at the last, without running past them", () => {
    expect(
      virtualWindow({ heights, scrollTop: 0, viewportHeight: 150, overscan: 3 }),
    ).toMatchObject({ start: 0, end: 5, offsetTop: 0 })

    const atEnd = virtualWindow({ heights, scrollTop: 900, viewportHeight: 100, overscan: 3 })
    expect(atEnd.start).toBe(6)
    expect(atEnd.end).toBe(10)
  })

  it("mounts everything while any row is still unmeasured", () => {
    // Guessing a missing height is how a virtualised transcript drops the
    // reader somewhere else on the next scroll; until every row is measured,
    // the honest window is all of them.
    const window = virtualWindow({
      heights: [100, undefined, 100],
      scrollTop: 0,
      viewportHeight: 50,
      overscan: 0,
    })

    expect(window.start).toBe(0)
    expect(window.end).toBe(3)
  })

  it("mounts everything when nothing has been laid out", () => {
    // jsdom and a first paint both report zero-height rows; the transcript must
    // render rather than compute an empty window from a box it cannot see.
    const unmeasured = virtualWindow({
      heights: [0, 0, 0],
      scrollTop: 0,
      viewportHeight: 0,
      overscan: 2,
    })

    expect(unmeasured.start).toBe(0)
    expect(unmeasured.end).toBe(3)
    expect(unmeasured.totalHeight).toBe(0)
  })

  it("never returns an empty window for a non-empty transcript", () => {
    const window = virtualWindow({
      heights: [40, 40],
      scrollTop: 10_000,
      viewportHeight: 100,
      overscan: 0,
    })

    expect(window.start).toBeLessThan(window.end)
    expect(window.end).toBe(2)
  })
})

/**
 * The wiring, not just the arithmetic: the plan asks the transcript to mount
 * only the visible turns, and a window nothing consumes is the arithmetic
 * alone. Until the transcript has measured its rows it mounts all of them,
 * which is also what a test environment without layout gets.
 */
describe("RunTimeline virtualisation", () => {
  const view = {
    ...createEmptyRunView(),
    timeline: [1, 2, 3].map((n) => ({
      type: "message" as const,
      key: `message-${n}`,
      sequence: n,
      occurredAt: "2026-09-20T06:00:00Z",
      turnId: null,
      role: "assistant" as const,
      text: `turn ${n}`,
      final: true,
    })),
  }

  it("marks each mounted turn as a virtualised row", () => {
    const wrapper = mount(RunTimeline, { props: { view } })

    const rows = wrapper.findAll("[data-virtualized-turn]")
    expect(rows).toHaveLength(3)
    expect(rows.map((row) => row.attributes("data-virtualized-turn"))).toEqual(["0", "1", "2"])
  })

  it("mounts fewer rows once the transcript has measured them and the reader scrolls", async () => {
    // jsdom reports no layout, so the heights are written by hand: this is the
    // one path the browser would take on a long governed run, and the one the
    // plan's "only visible turns mount" clause is about.
    const many = {
      ...createEmptyRunView(),
      timeline: Array.from({ length: 20 }, (_, n) => ({
        type: "message" as const,
        key: `message-${n}`,
        sequence: n,
        occurredAt: "2026-09-20T06:00:00Z",
        turnId: null,
        role: "assistant" as const,
        text: `turn ${n}`,
        final: true,
      })),
    }
    const wrapper = mount(RunTimeline, {
      props: { view: many, scrollTop: 0, viewportHeight: 400 },
    })
    const rows = wrapper.findAll("[data-virtualized-turn]")
    // Every row reports a 100px box, which is what the component measures.
    for (const row of rows) {
      Object.defineProperty(row.element, "getBoundingClientRect", {
        value: () => ({ height: 100 }),
      })
      Object.defineProperty(row.element, "offsetHeight", { value: 100 })
    }
    await wrapper.vm.$nextTick()
    // A second pass now that the rows have heights: the transcript re-measures
    // and the window closes around the viewport plus its overscan.
    await wrapper.setProps({ scrollTop: 1200 })
    await wrapper.vm.$nextTick()

    const windowed = wrapper.findAll("[data-virtualized-turn]")
    expect(windowed.length).toBeLessThan(20)
    expect(windowed.length).toBeGreaterThan(0)
    expect(Number(windowed[0]!.attributes("data-virtualized-turn"))).toBeGreaterThan(0)
    expect(wrapper.find("[data-virtual-spacer=\"top\"]").exists()).toBe(true)
  })
})
