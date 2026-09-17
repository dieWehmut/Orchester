import { h } from 'vue'
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppField from '../src/components/AppField.vue'

describe('AppField', () => {
  it('connects the label, hint, and error to the slotted control', () => {
    const wrapper = mount(AppField, {
      props: {
        id: 'provider-key',
        label: 'Provider key',
        hint: 'Stored in the local credential store.',
        error: 'A key is required.',
        required: true,
      },
      slots: {
        default: ({
          controlId,
          describedBy,
          invalid,
          required,
        }: {
          controlId: string
          describedBy: string | undefined
          invalid: boolean
          required: boolean
        }) =>
          h('input', {
            id: controlId,
            'aria-describedby': describedBy,
            'aria-invalid': invalid ? 'true' : undefined,
            'aria-required': required ? 'true' : undefined,
          }),
      },
    })

    expect(wrapper.get('label').attributes('for')).toBe('provider-key')
    expect(wrapper.get('#provider-key-hint').text()).toBe('Stored in the local credential store.')
    expect(wrapper.get('#provider-key-error').attributes('role')).toBe('alert')
    expect(wrapper.get('input').attributes()).toMatchObject({
      id: 'provider-key',
      'aria-describedby': 'provider-key-hint provider-key-error',
      'aria-invalid': 'true',
      'aria-required': 'true',
    })
  })

  it('omits optional descriptions and invalid state when they are absent', () => {
    const wrapper = mount(AppField, {
      props: { id: 'model', label: 'Model' },
      slots: {
        default: ({
          controlId,
          describedBy,
          invalid,
        }: {
          controlId: string
          describedBy: string | undefined
          invalid: boolean
        }) =>
          h('input', {
            id: controlId,
            'aria-describedby': describedBy,
            'aria-invalid': invalid ? 'true' : undefined,
          }),
      },
    })

    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    expect(wrapper.get('input').attributes('aria-describedby')).toBeUndefined()
    expect(wrapper.get('input').attributes('aria-invalid')).toBeUndefined()
  })
})
