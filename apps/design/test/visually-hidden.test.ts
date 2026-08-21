import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import VisuallyHidden from '../src/components/VisuallyHidden.vue'

describe('VisuallyHidden', () => {
  it('keeps its content in the accessibility tree while visually clipping it', () => {
    const wrapper = mount(VisuallyHidden, {
      slots: { default: 'Current run status' },
    })

    expect(wrapper.element.tagName).toBe('SPAN')
    expect(wrapper.text()).toBe('Current run status')
    expect(wrapper.classes()).toContain('visually-hidden')
    expect(wrapper.attributes('aria-hidden')).toBeUndefined()
  })

  it('supports a focusable variant for skip links and reveal-on-focus labels', () => {
    const wrapper = mount(VisuallyHidden, {
      props: { focusable: true },
      slots: { default: 'Skip to transcript' },
    })

    expect(wrapper.classes()).toContain('visually-hidden--focusable')
  })
})
