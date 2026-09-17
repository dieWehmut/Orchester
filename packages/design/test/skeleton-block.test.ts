import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'

import SkeletonBlock from '../src/components/SkeletonBlock.vue'

afterEach(() => {
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

describe('SkeletonBlock', () => {
  it('renders the requested number of layout-stable placeholder lines', () => {
    const wrapper = mount(SkeletonBlock, {
      props: { lines: 3, height: '12px' },
    })

    expect(wrapper.attributes('aria-hidden')).toBe('true')
    expect(wrapper.findAll('[data-skeleton-line]')).toHaveLength(3)
    expect(wrapper.find('[data-skeleton-line]').attributes('style')).toContain('height: 12px')
  })

  it('disables shimmer when reduced motion is requested', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    )

    const wrapper = mount(SkeletonBlock, { props: { lines: 1 } })
    await nextTick()

    expect(wrapper.classes()).toContain('skeleton-block--static')
  })
})
