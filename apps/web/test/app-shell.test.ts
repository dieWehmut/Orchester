import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import App from '../src/App.vue'

describe('WebUI app shell', () => {
  it('renders the product header and a single workspace main region', () => {
    const wrapper = mount(App)

    expect(wrapper.get('[data-testid="product-name"]').text()).toBe('Orchester')
    expect(wrapper.findAll('main')).toHaveLength(1)
    expect(wrapper.get('main').attributes('aria-label')).toBe('Agent workspace')
  })
})
