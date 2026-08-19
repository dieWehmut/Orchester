import { mount, type VueWrapper } from '@vue/test-utils'
import { createMemoryHistory } from 'vue-router'
import { afterEach, describe, expect, it } from 'vitest'

import ArchitectureView from '../src/views/ArchitectureView.vue'
import HomeView from '../src/views/HomeView.vue'
import InstallView from '../src/views/InstallView.vue'
import { createWebsiteRouter } from '../src/router'

const mountedWrappers: VueWrapper[] = []

async function mountView(component: typeof HomeView, path: string): Promise<VueWrapper> {
  const router = createWebsiteRouter(createMemoryHistory())
  await router.push(path)
  await router.isReady()
  const wrapper = mount(component, {
    attachTo: document.body,
    global: { plugins: [router] },
  })
  mountedWrappers.push(wrapper)
  return wrapper
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount()
  }
  document.body.replaceChildren()
  document.body.style.overflow = ''
})

describe('website content views', () => {
  it('presents a product hero and the three governance pillars', async () => {
    const wrapper = await mountView(HomeView, '/')

    expect(wrapper.get('main[data-page="home"] h1').text()).toContain('observable')
    expect(wrapper.get('[data-home-hero]')).toBeTruthy()
    expect(wrapper.get('[data-capability-grid]').findAll('article')).toHaveLength(3)
    expect(wrapper.get('[data-adapter-grid]').findAll('article')).toHaveLength(3)
    expect(wrapper.get('[data-governance-section]')).toBeTruthy()
    expect(wrapper.get('[data-home-hero] [data-site-link="/install"]')).toBeTruthy()
  })

  it('renders architecture stages from a complete typed flow', async () => {
    const wrapper = await mountView(ArchitectureView, '/architecture')

    expect(wrapper.get('main[data-page="architecture"] h1').text()).toContain('Architecture')
    expect(wrapper.get('[data-architecture-flow]').findAll('[data-architecture-stage]')).toHaveLength(4)
    expect(wrapper.get('[data-architecture-boundary]').text().toLowerCase()).toContain('loopback')
  })

  it('renders install steps with copyable shell commands', async () => {
    const wrapper = await mountView(InstallView, '/install')

    expect(wrapper.get('main[data-page="install"] h1').text()).toContain('Install')
    expect(wrapper.get('[data-install-steps]').findAll('[data-install-step]')).toHaveLength(3)
    expect(wrapper.get('[data-install-steps] code').text()).toContain('pnpm')
    expect(wrapper.get('[data-install-prerequisites]')).toBeTruthy()
  })
})
