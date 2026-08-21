import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import WindowChrome from '../src/components/layout/WindowChrome.vue'
import type { DesktopWindowController } from '../src/platform/desktop-window'

function fakeController(): DesktopWindowController & { calls: string[] } {
  const calls: string[] = []
  let maximized = false
  return {
    enabled: true,
    calls,
    minimize: vi.fn(async () => {
      calls.push('minimize')
    }),
    toggleMaximize: vi.fn(async () => {
      maximized = !maximized
      calls.push('toggleMaximize')
    }),
    close: vi.fn(async () => {
      calls.push('close')
    }),
    startDragging: vi.fn(async () => {
      calls.push('startDragging')
    }),
    isMaximized: vi.fn(async () => maximized),
  }
}

describe('WindowChrome', () => {
  it('does not render browser-only window controls', () => {
    const wrapper = mount(WindowChrome, {
      props: { controller: { enabled: false } as DesktopWindowController },
    })

    expect(wrapper.find('[data-window-chrome]').exists()).toBe(false)
  })

  it('renders accessible controls and forwards actions', async () => {
    const controller = fakeController()
    const wrapper = mount(WindowChrome, { props: { controller, title: 'Orchester' } })

    expect(wrapper.get('[data-tauri-drag-region]').attributes('aria-label')).toBe('Orchester')
    expect(wrapper.get('[data-window-action="minimize"]').attributes('aria-label')).toBe(
      'Minimize window',
    )

    await wrapper.get('[data-tauri-drag-region]').trigger('mousedown')
    await wrapper.get('[data-window-action="minimize"]').trigger('click')
    await wrapper.get('[data-window-action="maximize"]').trigger('click')
    await wrapper.get('[data-window-action="close"]').trigger('click')

    expect(controller.calls).toEqual(['startDragging', 'minimize', 'toggleMaximize', 'close'])
    expect(wrapper.get('[data-window-action="maximize"]').attributes('aria-label')).toBe(
      'Restore window',
    )
  })
})
