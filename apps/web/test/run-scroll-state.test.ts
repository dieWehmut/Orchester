import { describe, expect, it } from "vitest"

import { fadeDecision, readScrollState, stickDecision } from "../src/components/run/scroll-state"

/**
 * The transcript sticks to the bottom only while the reader is already there.
 * Getting that wrong is the one bug that makes a streaming transcript unusable:
 * it either yanks the page away from someone reading back, or it silently stops
 * following a run that is still producing output.
 */
describe("run transcript scroll state", () => {
  const box = (scrollTop: number, scrollHeight: number, clientHeight: number) => ({
    scrollTop,
    scrollHeight,
    clientHeight,
  })

  it("reports which ways the transcript can still scroll", () => {
    expect(readScrollState(box(0, 1000, 400))).toEqual({
      canScrollUp: false,
      canScrollDown: true,
      atBottom: false,
    })

    expect(readScrollState(box(600, 1000, 400))).toEqual({
      canScrollUp: true,
      canScrollDown: false,
      atBottom: true,
    })

    expect(readScrollState(box(300, 1000, 400))).toEqual({
      canScrollUp: true,
      canScrollDown: true,
      atBottom: false,
    })
  })

  it("treats content that fits as already at the bottom", () => {
    expect(readScrollState(box(0, 200, 400))).toEqual({
      canScrollUp: false,
      canScrollDown: false,
      atBottom: true,
    })
  })

  it("keeps sticking to the bottom only while the reader is at it", () => {
    expect(stickDecision("at-bottom", "appended")).toBe("stick")
    expect(stickDecision("reading-back", "appended")).toBe("hold")
  })

  it("follows again when the reader returns to the bottom", () => {
    expect(stickDecision("reading-back", "reached-bottom")).toBe("stick")
  })

  it("follows when the reader asks for the bottom from anywhere", () => {
    expect(stickDecision("reading-back", "jump-requested")).toBe("stick")
  })

  it("shows the top fade only once there is content above the reader", () => {
    // The fade under the header is the reader's only signal that the transcript
    // continues upward, so it may not be drawn on a transcript that is already
    // at its top. It rides the same flag the scroll contract already reports.
    expect(fadeDecision({ canScrollUp: false, canScrollDown: true, atBottom: false })).toBe("hidden")
    expect(fadeDecision({ canScrollUp: true, canScrollDown: true, atBottom: false })).toBe("visible")
    expect(fadeDecision({ canScrollUp: true, canScrollDown: false, atBottom: true })).toBe("visible")
    expect(fadeDecision({ canScrollUp: false, canScrollDown: false, atBottom: true })).toBe("hidden")
  })
})

