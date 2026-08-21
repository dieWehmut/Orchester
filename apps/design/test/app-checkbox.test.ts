import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppCheckbox from '../src/components/AppCheckbox.vue'

describe('AppCheckbox', () => {
  it('renders a labelled controlled checkbox and emits its next value', async () => {
    const wrapper = mount(AppCheckbox, {
      props: {
        modelValue: false,
        id: 'remember',
        label: 'Remember this workspace',
        describedBy: 'remember-hint',
        required: true,
      },
    })
    const input = wrapper.get('input')

    expect(wrapper.get('label').text()).toBe('Remember this workspace')
    expect(input.attributes()).toMatchObject({
      id: 'remember',
      type: 'checkbox',
      'aria-describedby': 'remember-hint',
      'aria-required': 'true',
    })
    expect((input.element as HTMLInputElement).checked).toBe(false)

    await input.setValue(true)

    expect(wrapper.emitted('update:modelValue')).toEqual([[true]])
  })

  it('sets indeterminate state and exposes invalid/disabled state', () => {
    const wrapper = mount(AppCheckbox, {
      props: {
        modelValue: false,
        label: 'Select all files',
        indeterminate: true,
        invalid: true,
        disabled: true,
      },
    })
    const input = wrapper.get('input').element as HTMLInputElement

    expect(input.indeterminate).toBe(true)
    expect(wrapper.get('input').attributes('aria-invalid')).toBe('true')
    expect(wrapper.get('input').attributes('disabled')).toBeDefined()
  })
})
