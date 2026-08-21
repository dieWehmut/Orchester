import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

export type DesktopWindowUnlisten = () => void

export interface DesktopWindowHandle {
  minimize: () => Promise<void>
  toggleMaximize: () => Promise<void>
  close: () => Promise<void>
  isMaximized: () => Promise<boolean>
  onResized: (listener: () => void) => Promise<DesktopWindowUnlisten>
  onFocusChanged: (listener: () => void) => Promise<DesktopWindowUnlisten>
}

export interface DesktopWindowController {
  readonly enabled: boolean
  minimize: () => Promise<void>
  toggleMaximize: () => Promise<void>
  close: () => Promise<void>
  isMaximized: () => Promise<boolean>
  listenMaximized: (listener: (maximized: boolean) => void) => DesktopWindowUnlisten
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
  isMaximized: async () => false,
  listenMaximized: () => () => undefined,
}

async function runWindowAction(action: () => Promise<void>): Promise<void> {
  try {
    await action()
  } catch {
    // Native window actions are best-effort UI affordances.
  }
}

function stopWindowListener(unlisten: DesktopWindowUnlisten): void {
  try {
    unlisten()
  } catch {
    // Listener cleanup should remain safe during component teardown.
  }
}

function listenToMaximizedState(
  window: DesktopWindowHandle,
  listener: (maximized: boolean) => void,
): DesktopWindowUnlisten {
  const unlisteners: DesktopWindowUnlisten[] = []
  let active = true
  let lastMaximized: boolean | undefined
  let syncSequence = 0

  async function syncMaximized(): Promise<void> {
    const sequence = ++syncSequence
    try {
      const maximized = await window.isMaximized()
      if (!active || sequence !== syncSequence || maximized === lastMaximized) return
      lastMaximized = maximized
      listener(maximized)
    } catch {
      // A failed state query must not escape from a native event callback.
    }
  }

  async function register(
    subscribe: (listener: () => void) => Promise<DesktopWindowUnlisten>,
  ): Promise<void> {
    try {
      const unlisten = await subscribe(() => {
        void syncMaximized()
      })
      if (active) unlisteners.push(unlisten)
      else stopWindowListener(unlisten)
    } catch {
      // One unavailable native event must not disable the other subscription.
    }
  }

  async function initialize(): Promise<void> {
    await Promise.all([
      register((eventListener) => window.onResized(eventListener)),
      register((eventListener) => window.onFocusChanged(eventListener)),
    ])
    await syncMaximized()
  }

  void initialize().catch(() => undefined)

  return () => {
    if (!active) return
    active = false
    for (const unlisten of unlisteners.splice(0)) stopWindowListener(unlisten)
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
    isMaximized: () => runtime.window!.isMaximized(),
    listenMaximized: (listener) => listenToMaximizedState(runtime.window!, listener),
  }
}

export const desktopWindow = createDesktopWindowController()
