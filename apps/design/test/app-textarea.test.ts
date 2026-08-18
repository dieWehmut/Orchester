import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppTextarea from '../src/components/AppTextarea.vue'

describe('AppTextarea', () => {
  it('renders a controlled value with stable rows and field semantics', async () => {
    const wrapper = mount(AppTextarea, {
      props: {
        modelValue: 'first line',
        id: 'instructions',
        rows: 6,
        describedBy: 'instructions-hint',
        required: true,
        placeholder: 'Describe the task',
      },
    })
    const textarea = wrapper.get('textarea')

    expect(textarea.element.value).toBe('first line')
    expect(textarea.attributes()).toMatchObject({
      id: 'instructions',
      rows: '6',
      'aria-describedby': 'instructions-hint',
      'aria-required': 'true',
      placeholder: 'Describe the task',
    })

    await textarea.setValue('second line')

    expect(wrapper.emitted('update:modelValue')).toEqual([['second line']])
  })

  it('marks invalid and disabled controls without inventing descriptions', () => {
    const wrapper = mount(AppTextarea, {
      props: { modelValue: '', id: 'notes', invalid: true, disabled: true },
    })
    const textarea = wrapper.get('textarea')

    expect(textarea.attributes('aria-invalid')).toBe('true')
    expect(textarea.attributes('aria-describedby')).toBeUndefined()
    expect(textarea.attributes('disabled')).toBeDefined()
  })
})
