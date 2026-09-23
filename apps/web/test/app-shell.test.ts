import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import App from '../src/App.vue'
import type { HttpClient } from '../src/api/http'
import type { DesktopWindowController } from '../src/platform/desktop-window'
import { createAppStores } from '../src/stores/app'

describe('WebUI app shell', () => {
  it('draws the title row above every route and leaves the main landmark to the view', () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const wrapper = mount(App, { global: { plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div data-testid="routed" />' }) }] } })

    // The row is above the outlet rather than inside a routed view: every route
    // gets the same top strip, and the shell still declares no main landmark.
    expect(wrapper.get('[data-title-row]').element.previousElementSibling).toBeNull()
    expect(wrapper.get('.app-shell__outlet [data-testid="routed"]')).toBeTruthy()
    expect(wrapper.findAll('main')).toHaveLength(0)
    expect(wrapper.find('[data-window-chrome]').exists()).toBe(false)
  })

  it('adds the window controls only for the desktop runtime', () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const wrapper = mount(App, {
      props: { desktopController: fakeDesktopWindow() },
      global: {
        plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div data-testid="routed" />' }) }],
      },
    })

    expect(wrapper.get('[data-title-row]')).toBeTruthy()
    expect(wrapper.get('[data-window-chrome]')).toBeTruthy()
    expect(wrapper.get('.app-shell').classes()).toContain('app-shell--desktop')
    expect(wrapper.findAll('[data-window-action]')).toHaveLength(3)
  })

  it('drops the connection readout once the runtime is ready', async () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const wrapper = mount(App, { global: { plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div data-testid="routed" />' }) }] } })

    await stores.start()
    await wrapper.vm.$nextTick()

    // Ready is the state the reader assumes, so the row carries no readout for
    // it: the strip says something only while there is something to say.
    expect(wrapper.find('[data-testid="connection-label"]').exists()).toBe(false)
  })

  it('stops application transports when the root component unmounts', () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const stop = vi.spyOn(stores, 'stop')
    const wrapper = mount(App, { global: { plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div data-testid="routed" />' }) }] } })

    wrapper.unmount()

    expect(stop).toHaveBeenCalledOnce()
  })
})

function fakeHttp(): HttpClient {
  return {
    request: async () => undefined,
    get: async (path: string) => {
      if (path === '/bootstrap') {
        return {
          schema_version: 1,
          service_version: '0.1.2',
          server_state: 'running',
          workspace: { selected: true, name: 'Orchester' },
        }
      }
      if (path === '/session') return { schema_version: 1, csrf_token: 'csrf', expires_at: 1_800_000_000 }
      return { schema_version: 1, items: [], next_cursor: null }
    },
    post: async () => ({ schema_version: 1, csrf_token: 'csrf', expires_at: 1_800_000_000 }),
    put: async () => undefined,
    patch: async () => undefined,
    delete: async () => undefined,
  } as HttpClient
}

function fakeDesktopWindow(): DesktopWindowController {
  return {
    enabled: true,
    minimize: async () => undefined,
    toggleMaximize: async () => undefined,
    close: async () => undefined,
    isMaximized: async () => false,
    listenMaximized: () => () => undefined,
  }
}
