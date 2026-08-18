import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import ProgressBar from '../src/components/ProgressBar.vue'

describe('ProgressBar', () => {
  it('clamps values above and below the accessible range', async () => {
    const wrapper = mount(ProgressBar, {
      props: { value: 130, max: 100, label: 'Upload progress' },
    })

    expect(wrapper.attributes()).toMatchObject({
      role: 'progressbar',
      'aria-label': 'Upload progress',
      'aria-valuemin': '0',
      'aria-valuemax': '100',
      'aria-valuenow': '100',
    })
    expect(wrapper.get('[data-progress-fill]').attributes('style')).toContain('width: 100%')

    await wrapper.setProps({ value: -5 })
    expect(wrapper.attributes('aria-valuenow')).toBe('0')
    expect(wrapper.get('[data-progress-fill]').attributes('style')).toContain('width: 0%')
  })

  it('uses a custom maximum for the visible percentage', () => {
    const wrapper = mount(ProgressBar, {
      props: { value: 2, max: 8, label: 'Validation progress', showValue: true },
    })

    expect(wrapper.attributes('aria-valuemax')).toBe('8')
    expect(wrapper.get('[data-progress-fill]').attributes('style')).toContain('width: 25%')
    expect(wrapper.text()).toContain('25%')
  })
})
