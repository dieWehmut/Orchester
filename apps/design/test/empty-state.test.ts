import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import EmptyState from '../src/components/EmptyState.vue'

describe('EmptyState', () => {
  it('renders a labelled empty state with optional action content', async () => {
    const wrapper = mount(EmptyState, {
      props: {
        title: 'No sessions yet',
        description: 'Start a run to create the first session.',
        actionLabel: 'New session',
      },
    })

    expect(wrapper.attributes('role')).toBe('status')
    expect(wrapper.get('h2').text()).toBe('No sessions yet')
    expect(wrapper.text()).toContain('Start a run to create the first session.')

    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('action')).toHaveLength(1)
  })

  it('supports a custom visual slot without requiring it', () => {
    const wrapper = mount(EmptyState, {
      props: { title: 'Nothing here' },
      slots: { visual: '<span data-visual>Empty icon</span>' },
    })

    expect(wrapper.find('[data-visual]').exists()).toBe(true)
  })
})
