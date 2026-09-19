import { describe, expect, it } from "vitest"

import {
  createShortcutRegistry,
  DEFAULT_SHORTCUTS,
  formatShortcut,
  matchShortcut,
  type ShortcutDefinition,
} from "../src/shortcuts/registry"

const definition = (overrides: Partial<ShortcutDefinition> = {}): ShortcutDefinition => ({
  id: "palette.open",
  label: "Open the command palette",
  group: "Composer",
  keys: ["Mod", "K"],
  ...overrides,
})

describe("shortcut keys", () => {
  it("names the platform modifier instead of hard-coding one", () => {
    expect(formatShortcut(definition().keys, "macos")).toBe("Cmd+K")
    expect(formatShortcut(definition().keys, "windows")).toBe("Ctrl+K")
    expect(formatShortcut(definition().keys, "linux")).toBe("Ctrl+K")
  })

  it("matches the platform modifier rather than either one", () => {
    const keys = definition().keys

    const onMac = { key: "k", metaKey: true, ctrlKey: false, shiftKey: false, altKey: false }
    const onWindows = { key: "k", metaKey: false, ctrlKey: true, shiftKey: false, altKey: false }

    expect(matchShortcut(keys, onMac, "macos")).toBe(true)
    expect(matchShortcut(keys, onWindows, "windows")).toBe(true)
    // Ctrl is a distinct modifier on a Mac, not an alias for Cmd.
    expect(matchShortcut(keys, onWindows, "macos")).toBe(false)
    expect(matchShortcut(keys, onMac, "windows")).toBe(false)
  })

  it("requires the modifiers to match exactly", () => {
    const keys = definition().keys

    expect(
      matchShortcut(keys, { key: "k", metaKey: false, ctrlKey: true, shiftKey: true, altKey: false }, "windows"),
    ).toBe(false)
    expect(
      matchShortcut(keys, { key: "k", metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }, "windows"),
    ).toBe(false)
  })
})

describe("shortcut registry", () => {
  it("lists what every component registered, and no more", () => {
    const registry = createShortcutRegistry()
    const remove = registry.register(definition())

    expect(registry.list().map((entry) => entry.id)).toEqual(["palette.open"])

    remove()
    expect(registry.list()).toEqual([])
  })

  it("refuses a second binding for the same shortcut", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())

    expect(() => registry.register(definition({ id: "palette.alias" }))).toThrow(/already bound/)
  })

  it("lets a component rebind its own shortcut", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())

    registry.register(definition({ keys: ["Mod", "P"] }))

    expect(registry.list()).toHaveLength(1)
    expect(registry.list()[0]?.keys).toEqual(["Mod", "P"])
  })

  it("reports the effective keys for every registered shortcut", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())

    registry.rebind("palette.open", ["Mod", "K", "Shift"])
    expect(registry.effectiveKeys("palette.open")).toEqual(["Mod", "K", "Shift"])
  })

  it("resets every rebinding back to the shipped default", () => {
    const defaultKeys = DEFAULT_SHORTCUTS.find((entry) => entry.id === "palette.open")?.keys
    expect(defaultKeys).toBeDefined()

    const registry = createShortcutRegistry()
    // Registered with the shipped keys, as a component that wants them does.
    registry.register(definition({ keys: defaultKeys ?? [] }))
    registry.rebind("palette.open", ["Mod", "J"])

    registry.resetAll()

    expect(registry.effectiveKeys("palette.open")).toEqual(defaultKeys)
  })

  it("ignores a rebinding for a shortcut nobody registered", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())

    registry.rebind("never.registered", ["Mod", "Z"])

    expect(registry.list().map((entry) => entry.id)).toEqual(["palette.open"])
    expect(registry.effectiveKeys("never.registered")).toBeUndefined()
  })

  it("keeps the shipped defaults free of duplicate bindings", () => {
    const seen = new Set<string>()
    for (const shortcut of DEFAULT_SHORTCUTS) {
      const serialized = shortcut.keys.join("+")
      expect(seen.has(serialized)).toBe(false)
      seen.add(serialized)
    }
  })
})
