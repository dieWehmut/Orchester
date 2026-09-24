import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'

import App from '../src/App.vue'
import type { DesktopWindowController } from '../src/platform/desktop-window'
import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'

/**
 * The window close chord, section 8 of the design spec.
 *
 * The spec says Mod+W closes the active tab, the window itself closes only
 * once the last tab does, and an active run has to be confirmed before the
 * window goes away. The chord belongs to the shell rather than to the strip:
 * a reader who presses it with focus in the composer expects the same thing to
 * happen, and a shortcut the editor lists has to answer from wherever focus is.
 */

function fakeDesktopWindow(
  overrides: Partial<DesktopWindowController> = {},
): DesktopWindowController & { calls: string[] } {
  const calls: string[] = []
  return {
    calls,
    enabled: true,
    platform: 'windows',
    minimize: async () => undefined,
    toggleMaximize: async () => undefined,
    close: async () => {
      calls.push('close')
    },
    isMaximized: async () => false,
    listenMaximized: () => () => undefined,
    ...overrides,
  }
}

function testRouter(): Router {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', redirect: { name: 'workspace' } },
      { path: '/workspace', name: 'workspace', component: WorkspaceView },
      { path: '/settings', name: 'settings', component: { template: '<p>settings</p>' } },
    ],
  })
}

// The shortcut registry is one per module, so a mount that is never torn down
// keeps answering the chords the next test presses.
const mounted: VueWrapper[] = []

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
})

async function mountWorkspace(controller: DesktopWindowController) {
  const router = testRouter()
  await router.push({ name: 'workspace' })
  await router.isReady()

  const stores = createAppStores()
  const wrapper = mount(App, {
    props: { desktopController: controller },
    global: { plugins: [stores, router] },
    attachTo: document.body,
  })
  await flushPromises()
  mounted.push(wrapper)
  return { stores, wrapper }
}

function press(key: string, modifiers: Partial<KeyboardEventInit> = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    key,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    bubbles: true,
    cancelable: true,
    ...modifiers,
  })
  window.dispatchEvent(event)
  return event
}

describe('the window close chord', () => {
  it('closes the active tab from anywhere, not only from the strip', async () => {
    const { wrapper } = await mountWorkspace(fakeDesktopWindow())

    await wrapper.get('[data-tabstrip-tab="inspector"]').trigger('click')
    await nextTick()
    expect(wrapper.get('[data-tabstrip-tab="inspector"]').attributes('aria-selected')).toBe('true')

    press('w', { ctrlKey: true })
    await nextTick()

    // The run is the product and stays; the closed tab's surface folds away.
    expect(wrapper.get('[data-tabstrip-tab="run"]').attributes('aria-selected')).toBe('true')
    expect(wrapper.find('[data-tabstrip-tab="inspector"]').exists()).toBe(false)
    // The surface that tab names is the panel's, and closing the tab closes it.
    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('collapsed')
  })

  it('answers the chord while the composer has focus', async () => {
    const controller = fakeDesktopWindow()
    const { wrapper } = await mountWorkspace(controller)

    const textarea = wrapper.get('textarea')
    const event = new KeyboardEvent('keydown', {
      key: 'w',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    })
    textarea.element.dispatchEvent(event)
    await flushPromises()

    // A chord cannot be typed as a character, so it stays a command wherever
    // focus is: the transcript is the active tab, so this one means the window.
    expect(event.defaultPrevented).toBe(true)
    expect(controller.calls).toEqual(['close'])
  })

  it('keeps the window open while a run is active, until the reader confirms', async () => {
    const controller = fakeDesktopWindow()
    const { stores, wrapper } = await mountWorkspace(controller)

    stores.run.lifecycle.value = 'running'
    await nextTick()

    press('w', { ctrlKey: true })
    await nextTick()

    // A run that is still working is not something to close on one keystroke.
    expect(controller.calls).toEqual([])
    expect(wrapper.get('[data-window-close-confirm]').isVisible()).toBe(true)

    await wrapper.get('[data-window-close-confirm="cancel"]').trigger('click')
    await nextTick()
    expect(controller.calls).toEqual([])
    expect(wrapper.find('[data-window-close-confirm]').exists()).toBe(false)
  })

  it('closes the window once the reader confirms an active run', async () => {
    const controller = fakeDesktopWindow()
    const { stores, wrapper } = await mountWorkspace(controller)

    stores.run.lifecycle.value = 'running'
    await nextTick()

    press('w', { ctrlKey: true })
    await nextTick()
    await wrapper.get('[data-window-close-confirm="accept"]').trigger('click')
    await flushPromises()

    expect(controller.calls).toEqual(['close'])
  })

  it('closes the window with no run to lose, without asking', async () => {
    const controller = fakeDesktopWindow()
    await mountWorkspace(controller)

    press('w', { ctrlKey: true })
    await flushPromises()

    expect(controller.calls).toEqual(['close'])
  })

  it('leaves the chord to the browser when there is no desktop window to close', async () => {
    const { wrapper } = await mountWorkspace({ ...fakeDesktopWindow(), enabled: false })
    const event = press('w', { ctrlKey: true })
    await nextTick()

    expect(event.defaultPrevented).toBe(false)
    expect(wrapper.find('[data-tabstrip-tab="inspector"]').exists()).toBe(true)
  })
})
