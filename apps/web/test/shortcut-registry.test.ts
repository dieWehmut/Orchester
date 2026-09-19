import { describe, expect, it } from "vitest"

import {
  captureShortcut,
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

  it("finds the shortcut an event is asking for", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())
    registry.register(definition({ id: "composer.focus", keys: ["Mod", "L"] }))

    const event = { key: "l", metaKey: false, ctrlKey: true, shiftKey: false, altKey: false }

    expect(registry.match(event, "windows")?.id).toBe("composer.focus")
    expect(registry.match({ ...event, key: "z" }, "windows")).toBeUndefined()
  })

  it("resolves a match through the effective keys, not the registered ones", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())
    registry.rebind("palette.open", ["Mod", "P"])

    const event = { key: "p", metaKey: true, ctrlKey: false, shiftKey: false, altKey: false }

    expect(registry.match(event, "macos")?.id).toBe("palette.open")
    expect(
      registry.match({ ...event, key: "k" }, "macos"),
    ).toBeUndefined()
  })

  it("tells a subscriber when the bindings change", () => {
    const registry = createShortcutRegistry()
    const seen: string[] = []
    const unsubscribe = registry.subscribe(() => seen.push("changed"))

    registry.register(definition())
    registry.rebind("palette.open", ["Mod", "J"])
    registry.resetAll()

    // Registering is a change too: an editor open while a component mounts has
    // to grow a row for it, or it lists less than the app answers to.
    expect(seen).toEqual(["changed", "changed", "changed"])

    unsubscribe()
    registry.rebind("palette.open", ["Mod", "Z"])
    expect(seen).toHaveLength(3)
  })
})

describe("shortcut capture", () => {
  const event = (key: string, modifiers: Partial<KeyboardEvent> = {}) =>
    ({
      key,
      metaKey: false,
      ctrlKey: false,
      shiftKey: false,
      altKey: false,
      ...modifiers,
    }) as KeyboardEvent

  it("captures the modifier as Mod so the binding stays portable", () => {
    expect(captureShortcut(event("k", { ctrlKey: true }), "windows")).toEqual(["Mod", "K"])
    expect(captureShortcut(event("k", { metaKey: true }), "macos")).toEqual(["Mod", "K"])
  })

  it("keeps the shift and alt modifiers it saw", () => {
    expect(captureShortcut(event("k", { ctrlKey: true, shiftKey: true }), "windows")).toEqual([
      "Mod",
      "Shift",
      "K",
    ])
    expect(captureShortcut(event("k", { metaKey: true, altKey: true }), "macos")).toEqual([
      "Mod",
      "Alt",
      "K",
    ])
  })

  it("refuses a chord with no modifier, which would swallow typing", () => {
    expect(captureShortcut(event("k"), "windows")).toBeUndefined()
  })

  it("refuses the modifiers on their own", () => {
    expect(captureShortcut(event("Control", { ctrlKey: true }), "windows")).toBeUndefined()
    expect(captureShortcut(event("Shift", { shiftKey: true }), "windows")).toBeUndefined()
  })

  it("names a letter by its upper-case form so the editor reads naturally", () => {
    expect(captureShortcut(event("b", { ctrlKey: true }), "windows")).toEqual(["Mod", "B"])
  })
})
