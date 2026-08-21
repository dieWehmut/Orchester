import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

export interface DesktopWindowHandle {
  minimize: () => Promise<void>
  toggleMaximize: () => Promise<void>
  close: () => Promise<void>
  startDragging: () => Promise<void>
  isMaximized: () => Promise<boolean>
}

export interface DesktopWindowController {
  readonly enabled: boolean
  minimize: () => Promise<void>
  toggleMaximize: () => Promise<void>
  close: () => Promise<void>
  startDragging: () => Promise<void>
  isMaximized: () => Promise<boolean>
}

export interface DesktopWindowRuntime {
  enabled: boolean
  window: DesktopWindowHandle | null
}

const browserWindow: DesktopWindowController = {
  enabled: false,
  minimize: async () => undefined,
  toggleMaximize: async () => undefined,
  close: async () => undefined,
  startDragging: async () => undefined,
  isMaximized: async () => false,
}

async function runWindowAction(action: () => Promise<void>): Promise<void> {
  try {
    await action()
  } catch {
    // Native window actions are best-effort UI affordances.
  }
}

export function createDesktopWindowController(
  runtime: DesktopWindowRuntime = {
    enabled: isTauri(),
    window: isTauri() ? getCurrentWindow() : null,
  },
): DesktopWindowController {
  if (!runtime.enabled || !runtime.window) return browserWindow

  return {
    enabled: true,
    minimize: () => runWindowAction(() => runtime.window!.minimize()),
    toggleMaximize: () => runWindowAction(() => runtime.window!.toggleMaximize()),
    close: () => runWindowAction(() => runtime.window!.close()),
    startDragging: () => runWindowAction(() => runtime.window!.startDragging()),
    isMaximized: () => runtime.window!.isMaximized(),
  }
}

export const desktopWindow = createDesktopWindowController()
