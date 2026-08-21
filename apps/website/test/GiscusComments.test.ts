import { mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { describe, expect, it } from 'vitest'

import GiscusComments from '../src/components/site/GiscusComments.vue'
import { parseGiscusConfig } from '../src/comments/giscus-config'

const publicConfig = parseGiscusConfig({
  VITE_GISCUS_REPO: 'dieWehmut/Orchester',
  VITE_GISCUS_REPO_ID: 'R_example',
  VITE_GISCUS_CATEGORY: 'Announcements',
  VITE_GISCUS_CATEGORY_ID: 'DIC_example',
})!

function createTestRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/architecture', component: { template: '<div />' } },
    ],
  })
}

describe('GiscusComments', () => {
  it('renders a configuration hint without injecting a third-party script', async () => {
    const router = createTestRouter()
    await router.push('/')
    await router.isReady()
    const wrapper = mount(GiscusComments, { global: { plugins: [router] } })

    expect(wrapper.get('[data-giscus-disabled]').text()).toContain('Comments are not enabled')
    expect(document.querySelector('script[src="https://giscus.app/client.js"]')).toBeNull()
    wrapper.unmount()
  })

  it('injects a lazy Giscus script only when the public config is complete', async () => {
    const router = createTestRouter()
    await router.push('/architecture')
    await router.isReady()
    const wrapper = mount(GiscusComments, {
      props: { config: publicConfig },
      global: { plugins: [router] },
    })
    await new Promise<void>((resolve) => window.setTimeout(resolve, 260))

    const script = wrapper.get('[data-giscus-container] script')
    expect(script.attributes('src')).toBe('https://giscus.app/client.js')
    expect(script.attributes('data-repo')).toBe('dieWehmut/Orchester')
    expect(script.attributes('data-category')).toBe('Announcements')
    expect(script.attributes('data-mapping')).toBe('specific')
    expect(script.attributes('data-term')).toBe('orchester:architecture')
    expect(script.attributes('data-loading')).toBe('lazy')

    wrapper.unmount()
  })
})
