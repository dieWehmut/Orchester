import { describe, expect, it, vi } from 'vitest'

import {
  createDesktopWindowController,
  desktopWindow,
  type DesktopWindowHandle,
} from '../src/platform/desktop-window'

function fakeWindow(): DesktopWindowHandle & {
  calls: string[]
} {
  const calls: string[] = []
  return {
    calls,
    minimize: vi.fn(async () => {
      calls.push('minimize')
    }),
    toggleMaximize: vi.fn(async () => {
      calls.push('toggleMaximize')
    }),
    close: vi.fn(async () => {
      calls.push('close')
    }),
    startDragging: vi.fn(async () => {
      calls.push('startDragging')
    }),
    isMaximized: vi.fn(async () => true),
  }
}

describe('desktop window adapter', () => {
  it('is inert in the browser runtime', async () => {
    expect(desktopWindow.enabled).toBe(false)
    await desktopWindow.minimize()
    await expect(desktopWindow.isMaximized()).resolves.toBe(false)
  })

  it('forwards the supported window actions to Tauri', async () => {
    const handle = fakeWindow()
    const controller = createDesktopWindowController({ enabled: true, window: handle })

    expect(controller.enabled).toBe(true)
    await controller.startDragging()
    await controller.minimize()
    await controller.toggleMaximize()
    await controller.close()
    await expect(controller.isMaximized()).resolves.toBe(true)

    expect(handle.calls).toEqual(['startDragging', 'minimize', 'toggleMaximize', 'close'])
  })
})
