import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppInput from '../src/components/AppInput.vue'

describe('AppInput', () => {
  it('renders a controlled value with field accessibility attributes', async () => {
    const wrapper = mount(AppInput, {
      props: {
        modelValue: 'claude',
        id: 'model',
        type: 'search',
        describedBy: 'model-hint model-error',
        invalid: true,
        required: true,
        placeholder: 'Choose a model',
      },
    })
    const input = wrapper.get('input')

    expect(input.element.value).toBe('claude')
    expect(input.attributes()).toMatchObject({
      id: 'model',
      type: 'search',
      'aria-describedby': 'model-hint model-error',
      'aria-invalid': 'true',
      'aria-required': 'true',
      placeholder: 'Choose a model',
    })

    await input.setValue('codex')

    expect(wrapper.emitted('update:modelValue')).toEqual([['codex']])
  })

  it('omits optional ARIA attributes and supports native disabled state', () => {
    const wrapper = mount(AppInput, {
      props: { modelValue: '', id: 'token', disabled: true },
    })
    const input = wrapper.get('input')

    expect(input.attributes('disabled')).toBeDefined()
    expect(input.attributes('aria-describedby')).toBeUndefined()
    expect(input.attributes('aria-invalid')).toBeUndefined()
    expect(input.attributes('aria-required')).toBeUndefined()
  })
})
