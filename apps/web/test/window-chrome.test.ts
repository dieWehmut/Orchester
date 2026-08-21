import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import WindowChrome from '../src/components/layout/WindowChrome.vue'
import type { DesktopWindowController } from '../src/platform/desktop-window'

function fakeController(options: { rejectActions?: boolean } = {}): DesktopWindowController & {
  calls: string[]
  emitMaximized: (value: boolean) => void
  unlisten: ReturnType<typeof vi.fn>
} {
  const calls: string[] = []
  const unlisten = vi.fn()
  let listener: ((maximized: boolean) => void) | null = null
  let maximized = false
  return {
    enabled: true,
    calls,
    emitMaximized: (value) => {
      maximized = value
      listener?.(value)
    },
    unlisten,
    minimize: vi.fn(async () => {
      calls.push('minimize')
      if (options.rejectActions) throw new Error('minimize failed')
    }),
    toggleMaximize: vi.fn(async () => {
      calls.push('toggleMaximize')
      if (options.rejectActions) throw new Error('toggle failed')
      maximized = !maximized
    }),
    close: vi.fn(async () => {
      calls.push('close')
      if (options.rejectActions) throw new Error('close failed')
    }),
    isMaximized: vi.fn(async () => maximized),
    listenMaximized: vi.fn((nextListener) => {
      listener = nextListener
      nextListener(maximized)
      return unlisten
    }),
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
    expect(
      wrapper.findAll('[data-window-action]').map((control) => control.attributes('data-window-action')),
    ).toEqual(['close', 'minimize', 'maximize'])
    expect(wrapper.get('[data-window-action="minimize"]').attributes('aria-label')).toBe(
      'Minimize window',
    )

    await wrapper.get('[data-tauri-drag-region]').trigger('mousedown')
    await wrapper.get('[data-window-action="minimize"]').trigger('click')
    await wrapper.get('[data-window-action="maximize"]').trigger('click')
    await wrapper.get('[data-window-action="close"]').trigger('click')

    expect(controller.calls).toEqual(['minimize', 'toggleMaximize', 'close'])
    expect(wrapper.get('[data-window-action="maximize"]').attributes('aria-label')).toBe(
      'Restore window',
    )
  })

  it('tracks native maximized changes and unsubscribes on unmount', async () => {
    const controller = fakeController()
    const wrapper = mount(WindowChrome, { props: { controller } })

    controller.emitMaximized(true)
    await wrapper.vm.$nextTick()

    expect(wrapper.get('[data-window-action="maximize"]').attributes('aria-label')).toBe(
      'Restore window',
    )

    wrapper.unmount()
    expect(controller.unlisten).toHaveBeenCalledOnce()
  })

  it('contains rejected controller actions', async () => {
    const controller = fakeController({ rejectActions: true })
    const wrapper = mount(WindowChrome, { props: { controller } })

    await wrapper.get('[data-window-action="minimize"]').trigger('click')
    await wrapper.get('[data-window-action="maximize"]').trigger('click')
    await wrapper.get('[data-window-action="close"]').trigger('click')
    await flushPromises()

    expect(controller.calls).toEqual(['minimize', 'toggleMaximize', 'close'])
  })
})
