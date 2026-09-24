import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'

import { createAppStores } from '../src/stores/app'
import { shortcutRegistry } from '../src/shortcuts'
import App from '../src/App.vue'
import WorkspaceView from '../src/views/WorkspaceView.vue'

/**
 * The shortcuts the workspace itself answers.
 *
 * The composer owns its keystrokes, but the shell owns the ones that act on
 * the panes around it. They are registered by the mounted view, so the editor
 * cannot list a shortcut that no mounted component would answer, and they are
 * driven here through a real keydown on the window rather than by calling the
 * handler directly.
 */
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

async function mountWorkspace(router: Router) {
  // The shell renders whatever the router resolved, so the route has to be
  // settled before the mount or the transcript never appears.
  await router.push({ name: 'workspace' })
  await router.isReady()

  const stores = createAppStores()
  // Mounted through the shell, because the shell owns the window listener:
  // a shortcut that only works when a test calls the handler is not a shortcut.
  return mount(App, {
    global: { plugins: [stores, router] },
    attachTo: document.body,
  })
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

describe('workspace shortcuts', () => {
  it('toggles the run surfaces from the keyboard', async () => {
    const wrapper = await mountWorkspace(testRouter())
    // The chord used to fold a right column; with no right column it opens and
    // closes the panel those surfaces live in.
    const panel = () => wrapper.get('[data-bottom-panel]')

    expect(panel().attributes('data-bottom-panel-state')).toBe('collapsed')

    press('b', { ctrlKey: true, altKey: true })
    await nextTick()
    expect(panel().attributes('data-bottom-panel-state')).toBe('expanded')

    press('b', { ctrlKey: true, altKey: true })
    await nextTick()
    expect(panel().attributes('data-bottom-panel-state')).toBe('collapsed')
  })

  it('folds the rail with the chord section 2.1 gives the collapse', async () => {
    // A column, not a drawer: below 1280 px the rail is the drawer the shell
    // opens on demand, and "collapsed" is a question about the column.
    const wide = window.innerWidth
    window.innerWidth = 1400
    try {
      const wrapper = await mountWorkspace(testRouter())
      const shell = () => wrapper.get('[data-rail-folded]')

      expect(shell().attributes('data-rail-folded')).toBe('false')
      expect(wrapper.find('[data-rail]').exists()).toBe(true)

      press('b', { ctrlKey: true })
      await nextTick()
      expect(shell().attributes('data-rail-folded')).toBe('true')
      expect(wrapper.find('[data-rail]').exists()).toBe(false)

      press('b', { ctrlKey: true })
      await nextTick()
      expect(shell().attributes('data-rail-folded')).toBe('false')
      expect(wrapper.find('[data-rail]').exists()).toBe(true)
      wrapper.unmount()
    } finally {
      window.innerWidth = wide
    }
  })

  it('opens the settings route from the keyboard', async () => {
    const router = testRouter()
    await mountWorkspace(router)

    press(',', { ctrlKey: true })
    // The push is a promise the handler does not await, so the route has to be
    // given a turn to settle before the assertion reads it.
    await flushPromises()

    expect(router.currentRoute.value.name).toBe('settings')
  })

  it('stops answering the shortcut once the view is unmounted', async () => {
    const wrapper = await mountWorkspace(testRouter())
    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('collapsed')

    wrapper.unmount()

    press('b', { ctrlKey: true, altKey: true })
    await nextTick()

    expect(shortcutRegistry.effectiveKeys('inspector.toggle')).toBeUndefined()
  })

  it('keeps the panel toggle out of the way while the reader is typing', async () => {
    const wrapper = await mountWorkspace(testRouter())
    const textarea = wrapper.get('textarea')
    await textarea.trigger('keydown', { key: 'b' })

    // A bare key is a character; the panel must not move under the reader.
    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('collapsed')
  })

  it('toggles the companion from the keyboard, and says so in the account menu', async () => {
    const { resetPetVisibilityForTests, usePetVisibility } = await import(
      '../src/features/pet/use-pet-visibility'
    )
    resetPetVisibilityForTests()
    const wrapper = await mountWorkspace(testRouter())
    const pet = usePetVisibility()
    expect(pet.visible.value).toBe(true)

    // The reference prints the chord on the menu row that performs it, so the
    // binding has to be one the registry really dispatches: a hint for a key
    // nothing answers would be worse than no hint at all.
    press('p', { ctrlKey: true, altKey: true })
    await nextTick()
    expect(pet.visible.value).toBe(false)

    press('p', { ctrlKey: true, altKey: true })
    await nextTick()
    expect(pet.visible.value).toBe(true)

    await wrapper.get('[data-rail-account-menu] [aria-haspopup="menu"]').trigger('click')
    const companionRow = wrapper
      .findAll('[role="menuitem"]')
      .find((row) => row.text().includes('Hide companion'))
    expect(companionRow?.text()).toContain('Ctrl+Alt+P')
    wrapper.unmount()
  })
})
