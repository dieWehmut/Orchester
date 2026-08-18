import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppSwitch from '../src/components/AppSwitch.vue'

describe('AppSwitch', () => {
  it('uses a native button with switch state and emits the next value', async () => {
    const wrapper = mount(AppSwitch, {
      props: {
        modelValue: false,
        label: 'Follow system theme',
        id: 'system-theme',
        describedBy: 'system-theme-hint',
      },
    })
    const control = wrapper.get('button')

    expect(control.element.tagName).toBe('BUTTON')
    expect(control.text()).toBe('Follow system theme')
    expect(control.attributes()).toMatchObject({
      id: 'system-theme',
      role: 'switch',
      'aria-checked': 'false',
      'aria-describedby': 'system-theme-hint',
      type: 'button',
    })

    await control.trigger('click')

    expect(wrapper.emitted('update:modelValue')).toEqual([[true]])
  })

  it('exposes checked, invalid, and disabled states', async () => {
    const wrapper = mount(AppSwitch, {
      props: {
        modelValue: true,
        label: 'Allow shell tools',
        invalid: true,
        disabled: true,
      },
    })
    const control = wrapper.get('button')

    expect(control.attributes('aria-checked')).toBe('true')
    expect(control.attributes('aria-invalid')).toBe('true')
    expect(control.attributes('disabled')).toBeDefined()

    await control.trigger('click')

    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  })
})
