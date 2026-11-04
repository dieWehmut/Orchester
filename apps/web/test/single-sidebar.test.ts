/** Navigation remains available independently of the session sidebar. */
import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import App from '../src/App.vue'
import { resetRailCollapsedForTests, useRailCollapsed } from '../src/composables/use-rail-collapsed'
import WorkspaceView from '../src/views/WorkspaceView.vue'
import { createAppStores } from '../src/stores/app'

function stores() {
  const appStores = createAppStores()
  appStores.agents.snapshot = AGENT_FLEET_FIXTURE
  appStores.agents.status = 'ready'
  return appStores
}

const Blank = { template: '<div />' }

function routerFor() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', name: 'workspace', component: Blank },
      { path: '/settings', name: 'settings', component: Blank },
    ],
  })
}

afterEach(() => { localStorage.clear(); resetRailCollapsedForTests() })

describe('persistent workspace navigation', () => {
  it('renders the destinations as rows inside the rail, and no strip', () => {
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores()] } })

    // The rows the reference draws under "New thread" live in the rail now.
    expect(wrapper.find('[data-rail-action="schedules"]').exists()).toBe(true)
    expect(wrapper.find('[data-rail-action="plugins"]').exists()).toBe(true)
    expect(wrapper.find('[data-rail-action="settings"]').exists()).toBe(true)
    expect(wrapper.find('[data-rail-section="footer"]').exists()).toBe(true)

    // The shell was told not to reserve a column for a strip it will not draw.
    expect(wrapper.get('.app-shell').attributes('data-navigation-in-rail')).toBe('true')
  })

  it('still draws the strip on a route that has not taken the rows', async () => {
    const router = routerFor()
    await router.push('/settings')
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [stores(), router] },
    })

    expect(wrapper.find('[data-workspace-navigation]').exists()).toBe(true)
    expect(wrapper.find('.app-shell__navigation').exists()).toBe(true)
  })

  it.each([false, true])('keeps destinations reachable on workspace with collapsed=%s', async (collapsed) => {
    useRailCollapsed().set(collapsed)
    const router = routerFor()
    await router.push('/')
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [stores(), router] },
    })

    expect(wrapper.find('[data-workspace-navigation]').exists()).toBe(true)
    expect(wrapper.find('.app-shell__navigation').exists()).toBe(true)
    await wrapper.get('[data-workspace-navigation] button[title="Settings"]').trigger('click')
    await router.isReady()
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(router.currentRoute.value.name).toBe('settings')
    expect(useRailCollapsed().collapsed.value).toBe(collapsed)
    wrapper.unmount()
  })
})
