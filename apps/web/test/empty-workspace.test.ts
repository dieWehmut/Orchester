import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import EmptyWorkspace from '../src/components/run/EmptyWorkspace.vue'

describe('EmptyWorkspace', () => {
  it('renders an original centered Orchester mark and concise work prompt', () => {
    const wrapper = mount(EmptyWorkspace)

    expect(wrapper.get('[data-orchester-mark]')).toBeTruthy()
    expect(wrapper.get('[data-empty-workspace]').attributes('aria-labelledby')).toBeTruthy()
    expect(wrapper.text()).toContain('What should we orchestrate?')
  })
})
