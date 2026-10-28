/**
 * The shell draws one sidebar, not a strip beside a list.
 *
 * The reference keeps "New thread", the lists and the destinations in a single
 * column. The workspace therefore renders those destinations itself and the
 * shell's own strip stands down there; every other route still gets the strip,
 * because those screens are a page with a list beside it. These tests pin both
 * halves of that arrangement, since the failure mode is silent: a strip left in
 * place squeezes the workspace into the remainder of a two-column body, and
 * nothing about the markup looks wrong.
 */
import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import App from '../src/App.vue'
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

describe('one sidebar column', () => {
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

  it('drops the strip on the workspace route, where the rail carries it', async () => {
    const router = routerFor()
    await router.push('/')
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [stores(), router] },
    })

    expect(wrapper.find('[data-workspace-navigation]').exists()).toBe(false)
    expect(wrapper.find('.app-shell__navigation').exists()).toBe(false)
  })
})
