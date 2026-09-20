import { describe, expect, it, vi } from "vitest"

import { createShortcutDispatch } from "../src/shortcuts/dispatch"
import { createShortcutRegistry, type ShortcutDefinition } from "../src/shortcuts/registry"

const definition = (overrides: Partial<ShortcutDefinition> = {}): ShortcutDefinition => ({
  id: "palette.open",
  labelKey: "shortcuts.labels.paletteOpen",
  groupKey: "shortcuts.groups.composer",
  keys: ["Mod", "K"],
  ...overrides,
})

/** A keyboard event the dispatcher can inspect without a real DOM. */
function keyEvent(key: string, modifiers: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return {
    key,
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    target: null,
    preventDefault: vi.fn(),
    ...modifiers,
  } as unknown as KeyboardEvent
}

describe("shortcut dispatch", () => {
  it("runs the handler for a registered shortcut", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())

    const dispatch = createShortcutDispatch({ registry, platform: () => "windows" })
    const handler = vi.fn()
    dispatch.bind("palette.open", handler)

    const event = keyEvent("k", { ctrlKey: true })
    dispatch.handle(event)

    expect(handler).toHaveBeenCalledTimes(1)
    expect(event.preventDefault).toHaveBeenCalled()
  })

  it("leaves an unregistered shortcut alone", () => {
    const registry = createShortcutRegistry()
    const dispatch = createShortcutDispatch({ registry, platform: () => "windows" })
    const handler = vi.fn()
    dispatch.bind("palette.open", handler)

    const event = keyEvent("k", { ctrlKey: true })
    dispatch.handle(event)

    expect(handler).not.toHaveBeenCalled()
    expect(event.preventDefault).not.toHaveBeenCalled()
  })

  it("ignores a bare-key shortcut while the reader is typing in a field", () => {
    // A key without a modifier is a character the reader is typing, not a
    // command: swallowing it would make the field impossible to write in.
    const registry = createShortcutRegistry()
    registry.register(definition({ id: "help.open", keys: ["?"] }))

    const dispatch = createShortcutDispatch({ registry, platform: () => "windows" })
    const handler = vi.fn()
    dispatch.bind("help.open", handler)

    const input = { tagName: "INPUT" } as unknown as EventTarget
    dispatch.handle(keyEvent("?", { shiftKey: true, target: input }))

    expect(handler).not.toHaveBeenCalled()
  })

  it("still fires a key chord while the reader is typing in a field", () => {
    // Ctrl+K inside a text field cannot be typed as text, so it stays a command.
    const registry = createShortcutRegistry()
    registry.register(definition())

    const dispatch = createShortcutDispatch({ registry, platform: () => "windows" })
    const handler = vi.fn()
    dispatch.bind("palette.open", handler)

    const input = { tagName: "INPUT" } as unknown as EventTarget
    dispatch.handle(keyEvent("k", { ctrlKey: true, target: input }))

    expect(handler).toHaveBeenCalledTimes(1)
  })

  it("stops answering an unbound shortcut once its handler is unbound", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())

    const dispatch = createShortcutDispatch({ registry, platform: () => "windows" })
    const handler = vi.fn()
    const unbind = dispatch.bind("palette.open", handler)
    unbind()

    dispatch.handle(keyEvent("k", { ctrlKey: true }))

    expect(handler).not.toHaveBeenCalled()
  })

  it("asks for the rebinding rather than the registered keys", () => {
    const registry = createShortcutRegistry()
    registry.register(definition())
    registry.rebind("palette.open", ["Mod", "P"])

    const dispatch = createShortcutDispatch({ registry, platform: () => "macos" })
    const handler = vi.fn()
    dispatch.bind("palette.open", handler)

    dispatch.handle(keyEvent("p", { metaKey: true }))

    expect(handler).toHaveBeenCalledTimes(1)
  })
})
