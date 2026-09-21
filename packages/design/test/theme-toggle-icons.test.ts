import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import ThemeToggle from '../src/components/ThemeToggle.vue'
import { resetAppearanceForTests } from '../src/composables/useAppearance'

afterEach(() => {
  resetAppearanceForTests()
  document.body.replaceChildren()
})

describe('ThemeToggle icons', () => {
  it('renders a Lucide icon with an accessible hidden presentation', () => {
    const wrapper = mount(ThemeToggle)
    const icon = wrapper.find('svg')

    expect(icon.classes()).toContain('lucide')
    expect(icon.attributes('aria-hidden')).toBe('true')
  })
})
