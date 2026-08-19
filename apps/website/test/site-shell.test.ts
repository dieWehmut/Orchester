import { mount, type VueWrapper } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createMemoryHistory } from 'vue-router'
import { afterEach, describe, expect, it } from 'vitest'

import SiteShell from '../src/components/site/SiteShell.vue'
import { createWebsiteRouter } from '../src/router'

async function mountShell(): Promise<{
  wrapper: VueWrapper
  router: ReturnType<typeof createWebsiteRouter>
}> {
  const router = createWebsiteRouter(createMemoryHistory())
  await router.push('/')
  await router.isReady()

  const wrapper = mount(SiteShell, {
    global: { plugins: [router] },
    slots: { default: '<main data-test-content>Content</main>' },
  })

  return { wrapper, router }
}

afterEach(() => {
  document.body.style.overflow = ''
})

describe('SiteShell', () => {
  it('keeps the page landmarks and primary links available', async () => {
    const { wrapper } = await mountShell()

    expect(wrapper.get('[data-site-header]').element.tagName).toBe('HEADER')
    expect(wrapper.get('nav[aria-label="Primary navigation"]')).toBeTruthy()
    expect(wrapper.get('main[data-test-content]').text()).toContain('Content')
    expect(wrapper.get('[data-site-footer]').element.tagName).toBe('FOOTER')
    expect(wrapper.get('[data-site-link="/architecture"]')).toBeTruthy()
  })

  it('opens the mobile navigation, closes on Escape, and restores focus', async () => {
    const { wrapper } = await mountShell()
    const trigger = wrapper.get('[data-mobile-nav-trigger]')

    await trigger.trigger('click')
    await nextTick()

    expect(wrapper.get('[data-mobile-nav]')).toBeTruthy()
    expect(document.activeElement).toBe(wrapper.get('[data-drawer-close]').element)
    expect(document.body.style.overflow).toBe('hidden')

    await wrapper.get('[data-mobile-nav] .app-drawer').trigger('keydown', { key: 'Escape' })
    await nextTick()

    expect(wrapper.find('[data-mobile-nav]').exists()).toBe(false)
    expect(document.activeElement).toBe(trigger.element)
    expect(document.body.style.overflow).toBe('')
  })

  it('closes the mobile navigation after following a route link', async () => {
    const { wrapper, router } = await mountShell()
    await wrapper.get('[data-mobile-nav-trigger]').trigger('click')

    await wrapper.get('[data-mobile-nav] [data-site-link="/install"]').trigger('click')
    await router.isReady()
    await nextTick()

    expect(router.currentRoute.value.name).toBe('install')
    expect(wrapper.find('[data-mobile-nav]').exists()).toBe(false)
  })
})
