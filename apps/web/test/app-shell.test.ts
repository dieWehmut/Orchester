import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import App from '../src/App.vue'
import type { HttpClient } from '../src/api/http'
import type { DesktopWindowController } from '../src/platform/desktop-window'
import { createAppStores } from '../src/stores/app'

describe('WebUI app shell', () => {
  it('renders the product header and a single workspace main region', () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const wrapper = mount(App, { global: { plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div />' }) }] } })

    expect(wrapper.get('[data-testid="product-name"]').text()).toBe('Orchester')
    expect(wrapper.findAll('main')).toHaveLength(1)
    expect(wrapper.get('main').attributes('aria-label')).toBe('Agent workspace')
    expect(wrapper.find('[data-window-chrome]').exists()).toBe(false)
  })

  it('mounts the custom titlebar only for the desktop runtime', () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const wrapper = mount(App, {
      props: { desktopController: fakeDesktopWindow() },
      global: {
        plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div />' }) }],
      },
    })

    expect(wrapper.get('[data-window-chrome]')).toBeTruthy()
    expect(wrapper.get('.app-shell').classes()).toContain('app-shell--desktop')
  })

  it('starts the runtime bootstrap after mount and reflects a ready connection', async () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const wrapper = mount(App, { global: { plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div />' }) }] } })

    await stores.start()
    await wrapper.vm.$nextTick()

    expect(wrapper.get('[data-testid="connection-label"]').text()).toBe('Connected')
    expect(wrapper.get('[data-testid="workspace-name"]').text()).toBe('Orchester')
  })

  it('stops application transports when the root component unmounts', () => {
    const stores = createAppStores({ http: fakeHttp(), agentStatusStreamFactory: null })
    const stop = vi.spyOn(stores, 'stop')
    const wrapper = mount(App, { global: { plugins: [stores, { install: (app) => app.component('RouterView', { template: '<div />' }) }] } })

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
    startDragging: async () => undefined,
    isMaximized: async () => false,
  }
}
