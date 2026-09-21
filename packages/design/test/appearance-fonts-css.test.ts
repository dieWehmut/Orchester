import { describe, expect, it } from "vitest"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

const tokens = readFileSync(resolve(process.cwd(), "src/tokens.css"), "utf8")

/**
 * The font and rail axes are stored, exposed, and written to the document by the
 * TypeScript layer; these rules are what make them visible. A stored value no
 * rule reads is a switch that does nothing.
 *
 * The axes re-point the existing public faces rather than adding parallel ones,
 * so every component that already asks for `--font-body`/`--font-mono` follows
 * the setting without being edited.
 */
describe("ui font axis tokens", () => {
  it("keeps the two alternative faces behind their own tokens", () => {
    expect(tokens).toContain("--font-ui-sans:")
    expect(tokens).toContain("--font-ui-serif:")
  })

  it("re-points the interface face per data-ui-font value", () => {
    expect(tokens).toMatch(
      /\[data-ui-font='sans'\][^{]*\{[^}]*--font-body:\s*var\(--font-ui-sans\)/,
    )
    expect(tokens).toMatch(
      /\[data-ui-font='serif'\][^{]*\{[^}]*--font-body:\s*var\(--font-ui-serif\)/,
    )
  })

  it("leaves the stack alone for the system default", () => {
    expect(tokens).not.toMatch(/\[data-ui-font='system'\][^{]*\{[^}]*--font-body:/)
  })
})

describe("content font axis tokens", () => {
  it("resolves the ui content face to the interface face, not a second family", () => {
    expect(tokens).toMatch(
      /\[data-content-font='ui'\][^{]*\{[^}]*--font-mono:\s*var\(--font-body\)/,
    )
  })

  it("leaves the mono stack alone for the mono default", () => {
    expect(tokens).not.toMatch(/\[data-content-font='mono'\][^{]*\{[^}]*--font-mono:/)
  })
})

describe("rail appearance axis tokens", () => {
  it("keeps the rail face behind a swappable token pair", () => {
    expect(tokens).toContain("--rail-surface:")
    expect(tokens).toContain("--rail-blur:")
  })

  it("defaults the rail to opaque with no blur", () => {
    expect(tokens).toMatch(/--rail-surface:\s*var\(--color-bg-surface\)/)
    expect(tokens).toMatch(/--rail-blur:\s*none/)
  })

  it("tints and blurs only when the rail is translucent", () => {
    expect(tokens).toMatch(
      /\[data-rail-appearance='translucent'\][^{]*\{[^}]*--rail-surface:\s*color-mix\(in srgb, var\(--color-bg-surface\)[^;]*;/,
    )
    expect(tokens).toMatch(
      /\[data-rail-appearance='translucent'\][^{]*\{[^}]*--rail-blur:\s*blur\(/,
    )
  })

  it("drops the blur where the surface cannot composite behind the window", () => {
    expect(tokens).toMatch(
      /\[data-rail-appearance='translucent'\]\[data-orchester-surface='web'\][^{]*\{[^}]*--rail-blur:\s*none/,
    )
  })
})
