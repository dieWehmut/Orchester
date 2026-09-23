import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import TitleRow from '../src/components/layout/TitleRow.vue'
import type { DesktopWindowController } from '../src/platform/desktop-window'
import {
  clearShellActionsForTests,
  registerShellAction,
} from '../src/components/layout/shell-actions'
import { resetRailCollapsedForTests } from '../src/composables/use-rail-collapsed'
import { shortcutRegistry } from '../src/shortcuts'

/**
 * The title row, region A of the shell.
 *
 * The reference draws one strip above everything: the rail's toggle, back and
 * forward, the four menus, and the window's captions at the trailing edge. Two
 * of those control kinds exist only where a native window does, so the row's
 * contract is both things at once - the order every face shares, and what the
 * desktop face adds to it. The chrome's older contract lives here rather than
 * in a file of its own because the row is where the chrome went.
 */

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

function row(controller: DesktopWindowController, connection?: 'pending' | 'ready' | 'offline' | 'error') {
  return mount(TitleRow, {
    props: connection === undefined ? { controller } : { controller, connection },
  })
}

describe('the title row', () => {
  it('draws the rail toggle, the arrows and the four menus in the reference order', () => {
    const wrapper = row({ enabled: false } as DesktopWindowController)

    expect(wrapper.find('[data-rail-toggle]').exists()).toBe(true)
    expect(wrapper.findAll('[data-title-nav]').map((node) => node.attributes('data-title-nav'))).toEqual([
      'back',
      'forward',
    ])
    expect(wrapper.findAll('[data-title-menu]').map((node) => node.attributes('data-title-menu'))).toEqual([
      'file',
      'edit',
      'view',
      'help',
    ])

    // The order is the reference's: the toggle leftmost, the menus between the
    // arrows and whatever else the row carries.
    const order = Array.from(
      wrapper.get('[data-title-row]').element.querySelectorAll(
        '[data-rail-toggle], [data-title-nav], [data-title-menu]',
      ),
    ).map((element) => element.getAttribute('data-title-menu') ?? element.getAttribute('data-title-nav') ?? 'rail')
    expect(order).toEqual(['rail', 'back', 'forward', 'file', 'edit', 'view', 'help'])
    wrapper.unmount()
  })

  it('draws the row in the browser too, and hides what only a window can answer', () => {
    const wrapper = row({ enabled: false } as DesktopWindowController)

    expect(wrapper.find('[data-title-row]').exists()).toBe(true)
    expect(wrapper.find('[data-window-chrome]').exists()).toBe(false)
    expect(wrapper.findAll('[data-window-action]')).toHaveLength(0)
    expect(wrapper.find('[data-tauri-drag-region]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('keeps an opaque surface when the native runtime offers no translucency', () => {
    const wrapper = row(Object.assign(fakeController(), { platform: 'windows' as const }))
    expect(wrapper.get('[data-title-row]').attributes('data-window-material')).toBe('opaque')
    wrapper.unmount()
  })

  it('leaves drag-region double clicks to Tauri so the window is not toggled twice', async () => {
    const controller = fakeController()
    const wrapper = row(controller)
    await wrapper.get('[data-tauri-drag-region]').trigger('dblclick')
    expect(controller.calls).toEqual([])
    wrapper.unmount()
  })

  it('carries the product name as the drag region tooltip rather than a second title', () => {
    const wrapper = row(fakeController())

    const drag = wrapper.get('[data-tauri-drag-region]')
    expect(drag.attributes('title')).toBe('Orchester')
    // The reference's window title would be a third place to keep the product
    // name in step with the rail and the tab strip, so the row names itself.
    expect(wrapper.get('[data-title-row]').text()).not.toContain('Orchester')
    wrapper.unmount()
  })

  it('places Windows captions after the drag region in native button order', () => {
    const wrapper = row(Object.assign(fakeController(), { platform: 'windows' as const }))

    expect(wrapper.get('[data-title-row]').attributes('data-window-platform')).toBe('windows')
    expect(wrapper.findAll('[data-window-action]').map((button) => button.attributes('data-window-action')))
      .toEqual(['minimize', 'maximize', 'close'])
    expect(wrapper.get('[data-window-action="maximize"]').attributes('data-native-snap-target')).toBe('true')
    expect(wrapper.get('[data-window-action="maximize"]').element.closest('[data-tauri-drag-region]')).toBeNull()
    wrapper.unmount()
  })

  it('reserves the macOS native traffic lights without drawing duplicate controls', () => {
    const wrapper = row(Object.assign(fakeController(), { platform: 'macos' as const }))

    expect(wrapper.get('[data-title-row]').attributes('data-window-platform')).toBe('macos')
    expect(wrapper.findAll('[data-window-action]')).toHaveLength(0)
    expect(wrapper.get('[data-native-traffic-lights]').attributes('aria-hidden')).toBe('true')
    expect(wrapper.get('[data-tauri-drag-region]').find('[data-window-action]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('renders accessible controls and forwards actions', async () => {
    const controller = fakeController()
    const wrapper = row(controller)

    expect(wrapper.get('[data-window-action="minimize"]').attributes('aria-label')).toBe('Minimize window')

    await wrapper.get('[data-window-action="minimize"]').trigger('click')
    await wrapper.get('[data-window-action="maximize"]').trigger('click')
    await wrapper.get('[data-window-action="close"]').trigger('click')

    expect(controller.calls).toEqual(['minimize', 'toggleMaximize', 'close'])
    expect(wrapper.get('[data-window-action="maximize"]').attributes('aria-label')).toBe('Restore window')
    wrapper.unmount()
  })

  it('tracks native maximized changes and unsubscribes on unmount', async () => {
    const controller = fakeController()
    const wrapper = row(controller)

    controller.emitMaximized(true)
    await wrapper.vm.$nextTick()

    expect(wrapper.get('[data-window-action="maximize"]').attributes('aria-label')).toBe('Restore window')

    wrapper.unmount()
    expect(controller.unlisten).toHaveBeenCalledOnce()
  })

  it('contains rejected controller actions', async () => {
    const controller = fakeController({ rejectActions: true })
    const wrapper = row(controller)

    await wrapper.get('[data-window-action="minimize"]').trigger('click')
    await wrapper.get('[data-window-action="maximize"]').trigger('click')
    await wrapper.get('[data-window-action="close"]').trigger('click')
    await flushPromises()

    expect(controller.calls).toEqual(['minimize', 'toggleMaximize', 'close'])
    wrapper.unmount()
  })
})

describe('the menus the row opens', () => {
  async function openMenu(wrapper: ReturnType<typeof row>, name: string): Promise<void> {
    await wrapper.get('[data-title-menu="' + name + '"] .app-menu__trigger').trigger('click')
  }

  function rowNamed(wrapper: ReturnType<typeof row>, name: string, menu: string) {
    return wrapper
      .findAll('[data-title-menu="' + menu + '"] .app-menu__item')
      .find((node) => node.text().includes(name))
  }

  it('offers the theme row the appearance screen owns', async () => {
    const wrapper = row({ enabled: false } as DesktopWindowController)

    await openMenu(wrapper, 'view')

    expect(rowNamed(wrapper, 'Switch to', 'view')).toBeTruthy()
    wrapper.unmount()
  })

  it('draws a row disabled with its reason while nothing answers the action', async () => {
    clearShellActionsForTests()
    resetRailCollapsedForTests()
    const wrapper = row({ enabled: false } as DesktopWindowController)

    await openMenu(wrapper, 'view')
    const inspector = rowNamed(wrapper, 'Toggle the inspector', 'view')

    expect(inspector).toBeTruthy()
    expect(inspector?.attributes('disabled')).toBeDefined()
    expect(inspector?.text()).toContain('Not on this screen')
    wrapper.unmount()
    clearShellActionsForTests()
  })

  it('prints no chord while the surface has bound none', async () => {
    clearShellActionsForTests()
    resetRailCollapsedForTests()
    const unregister = registerShellAction('inspector.toggle', () => undefined)
    const wrapper = row({ enabled: false } as DesktopWindowController)

    await openMenu(wrapper, 'view')
    const inspector = rowNamed(wrapper, 'Toggle the inspector', 'view')

    // The action exists, so the row is offered - but the reader is not taught a
    // key that would do nothing, because no mounted surface has bound one.
    expect(inspector?.attributes('disabled')).toBeUndefined()
    expect(inspector?.text()).toBe('Toggle the inspector')

    unregister()
    wrapper.unmount()
    clearShellActionsForTests()
  })

  it('teaches the chord the registry holds rather than a written-out default', async () => {
    clearShellActionsForTests()
    resetRailCollapsedForTests()
    const unregisterAction = registerShellAction('inspector.toggle', () => undefined)
    const unregisterShortcut = shortcutRegistry.register({
      id: 'inspector.toggle',
      labelKey: 'shortcuts.labels.inspectorToggle',
      groupKey: 'shortcuts.groups.layout',
      keys: ['Mod', 'Alt', 'B'],
    })
    const wrapper = row({ enabled: false } as DesktopWindowController)

    await openMenu(wrapper, 'view')
    expect(rowNamed(wrapper, 'Toggle the inspector', 'view')?.text()).toContain('Ctrl+Alt+B')
    await openMenu(wrapper, 'view')

    // The settings editor can rebind every one of these, and a menu that kept
    // the shipped chord would teach a key that no longer does anything.
    shortcutRegistry.rebind('inspector.toggle', ['Mod', 'J'])
    await wrapper.vm.$nextTick()
    await openMenu(wrapper, 'view')
    expect(rowNamed(wrapper, 'Toggle the inspector', 'view')?.text()).toContain('Ctrl+J')

    unregisterShortcut()
    unregisterAction()
    wrapper.unmount()
    clearShellActionsForTests()
  })

  it('reaches the rail through the surface that registered it', async () => {
    clearShellActionsForTests()
    resetRailCollapsedForTests()
    let folds = 0
    const unregister = registerShellAction('rail.toggle', () => {
      folds += 1
    })
    const wrapper = row({ enabled: false } as DesktopWindowController)

    await wrapper.get('[data-rail-toggle]').trigger('click')

    expect(folds).toBe(1)
    unregister()
    wrapper.unmount()
    clearShellActionsForTests()
  })
})
