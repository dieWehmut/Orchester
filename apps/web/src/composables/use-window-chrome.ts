import { onMounted, onUnmounted, ref } from 'vue'

import type {
  DesktopWindowController,
  DesktopWindowUnlisten,
} from '../platform/desktop-window'

const noopUnlisten: DesktopWindowUnlisten = () => undefined

export function useWindowChrome(controller: DesktopWindowController) {
  const maximized = ref(false)
  let unlisten = noopUnlisten

  onMounted(() => {
    if (!controller.enabled) return
    try {
      unlisten = controller.listenMaximized((value) => {
        maximized.value = value
      })
    } catch {
      unlisten = noopUnlisten
    }
  })

  onUnmounted(() => {
    try {
      unlisten()
    } catch {
      // Component teardown must continue even if native cleanup fails.
    }
    unlisten = noopUnlisten
  })

  async function minimize(): Promise<void> {
    try {
      await controller.minimize()
    } catch {
      // Window controls are best-effort and must not reject Vue event handlers.
    }
  }

  async function toggleMaximize(): Promise<void> {
    try {
      await controller.toggleMaximize()
      maximized.value = await controller.isMaximized()
    } catch {
      // Native events will provide the next available state after a failure.
    }
  }

  async function close(): Promise<void> {
    try {
      await controller.close()
    } catch {
      // Window controls are best-effort and must not reject Vue event handlers.
    }
  }

  return {
    close,
    maximized,
    minimize,
    toggleMaximize,
  }
}
