import { describe, expect, it, vi } from 'vitest'

import {
  createDesktopWindowController,
  desktopWindow,
  type DesktopWindowHandle,
} from '../src/platform/desktop-window'

function fakeWindow(): DesktopWindowHandle & {
  calls: string[]
  emitFocusChanged: () => void
  emitResized: () => void
  focusUnlisten: ReturnType<typeof vi.fn>
  resizeUnlisten: ReturnType<typeof vi.fn>
  setMaximized: (value: boolean) => void
} {
  const calls: string[] = []
  const focusListeners: Array<() => void> = []
  const resizedListeners: Array<() => void> = []
  const focusUnlisten = vi.fn()
  const resizeUnlisten = vi.fn()
  let maximized = true
  return {
    calls,
    emitFocusChanged: () => focusListeners.forEach((listener) => listener()),
    emitResized: () => resizedListeners.forEach((listener) => listener()),
    focusUnlisten,
    resizeUnlisten,
    setMaximized: (value) => {
      maximized = value
    },
    minimize: vi.fn(async () => {
      calls.push('minimize')
    }),
    toggleMaximize: vi.fn(async () => {
      calls.push('toggleMaximize')
    }),
    close: vi.fn(async () => {
      calls.push('close')
    }),
    isMaximized: vi.fn(async () => maximized),
    onFocusChanged: vi.fn(async (listener: () => void) => {
      focusListeners.push(listener)
      return focusUnlisten
    }),
    onResized: vi.fn(async (listener: () => void) => {
      resizedListeners.push(listener)
      return resizeUnlisten
    }),
  }
}

describe('desktop window adapter', () => {
  it('is inert in the browser runtime', async () => {
    expect(desktopWindow.enabled).toBe(false)
    await desktopWindow.minimize()
    await expect(desktopWindow.isMaximized()).resolves.toBe(false)
    expect(() => desktopWindow.listenMaximized(vi.fn())()).not.toThrow()
  })

  it('forwards the supported window actions to Tauri', async () => {
    const handle = fakeWindow()
    const controller = createDesktopWindowController({ enabled: true, window: handle })

    expect(controller.enabled).toBe(true)
    await controller.minimize()
    await controller.toggleMaximize()
    await controller.close()
    await expect(controller.isMaximized()).resolves.toBe(true)

    expect(handle.calls).toEqual(['minimize', 'toggleMaximize', 'close'])
  })

  it('absorbs native window action failures', async () => {
    const handle = fakeWindow()
    handle.minimize = vi.fn(async () => {
      throw new Error('minimize failed')
    })
    handle.toggleMaximize = vi.fn(async () => {
      throw new Error('toggle failed')
    })
    handle.close = vi.fn(async () => {
      throw new Error('close failed')
    })
    const controller = createDesktopWindowController({ enabled: true, window: handle })

    await expect(controller.minimize()).resolves.toBeUndefined()
    await expect(controller.toggleMaximize()).resolves.toBeUndefined()
    await expect(controller.close()).resolves.toBeUndefined()
  })

  it('syncs maximized state from native resize and focus events', async () => {
    const handle = fakeWindow()
    handle.setMaximized(false)
    const controller = createDesktopWindowController({ enabled: true, window: handle })
    const states: boolean[] = []

    const unlisten = controller.listenMaximized((maximized) => states.push(maximized))

    await vi.waitFor(() => expect(states).toEqual([false]))
    handle.setMaximized(true)
    handle.emitResized()
    await vi.waitFor(() => expect(states).toEqual([false, true]))
    handle.setMaximized(false)
    handle.emitFocusChanged()
    await vi.waitFor(() => expect(states).toEqual([false, true, false]))

    unlisten()
    unlisten()

    expect(handle.resizeUnlisten).toHaveBeenCalledOnce()
    expect(handle.focusUnlisten).toHaveBeenCalledOnce()
  })

  it('cleans up native listeners that finish subscribing after disposal', async () => {
    const handle = fakeWindow()
    const resized = deferred<() => void>()
    const focused = deferred<() => void>()
    const resizeUnlisten = vi.fn()
    const focusUnlisten = vi.fn()
    handle.onResized = vi.fn(() => resized.promise)
    handle.onFocusChanged = vi.fn(() => focused.promise)
    const controller = createDesktopWindowController({ enabled: true, window: handle })

    const unlisten = controller.listenMaximized(vi.fn())
    unlisten()
    resized.resolve(resizeUnlisten)
    focused.resolve(focusUnlisten)

    await vi.waitFor(() => {
      expect(resizeUnlisten).toHaveBeenCalledOnce()
      expect(focusUnlisten).toHaveBeenCalledOnce()
    })
  })
})

function deferred<T>(): {
  promise: Promise<T>
  resolve: (value: T) => void
} {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolver) => {
    resolve = resolver
  })
  return { promise, resolve }
}
