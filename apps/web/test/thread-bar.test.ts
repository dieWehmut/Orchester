import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ThreadBar from '../src/components/layout/ThreadBar.vue'

describe('ThreadBar', () => {
  it('renders the thread title with the share and panel controls', () => {
    const wrapper = mount(ThreadBar, {
      props: {
        title: 'Continue the shell work',
        shareLabel: 'Share',
        moreLabel: 'More options',
        panelLabel: 'Toggle inspector',
        panelOpen: true,
      },
    })

    expect(wrapper.get('[data-thread-title]').text()).toBe('Continue the shell work')
    expect(wrapper.get('[data-thread-action="share"]').attributes('aria-label')).toBe('Share')
    expect(wrapper.get('[data-thread-action="more"]').attributes('aria-label')).toBe('More options')
    const panel = wrapper.get('[data-thread-action="panel"]')
    expect(panel.attributes('aria-label')).toBe('Toggle inspector')
    expect(panel.attributes('aria-pressed')).toBe('true')
  })

  it('emits the panel, share, and more intents', async () => {
    const wrapper = mount(ThreadBar, {
      props: {
        title: 'New chat',
        shareLabel: 'Share',
        moreLabel: 'More options',
        panelLabel: 'Toggle inspector',
        panelOpen: false,
      },
    })

    await wrapper.get('[data-thread-action="panel"]').trigger('click')
    await wrapper.get('[data-thread-action="share"]').trigger('click')
    await wrapper.get('[data-thread-action="more"]').trigger('click')

    expect(wrapper.emitted('togglePanel')).toHaveLength(1)
    expect(wrapper.emitted('share')).toHaveLength(1)
    expect(wrapper.emitted('more')).toHaveLength(1)
    expect(wrapper.get('[data-thread-action="panel"]').attributes('aria-pressed')).toBe('false')
  })
})
