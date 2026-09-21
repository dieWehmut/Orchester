import { h, type Component } from 'vue'
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import {
  AppField,
  AppInput,
  AppSelect,
  AppTextarea,
  type AppFieldControlProps,
} from '../src'

const controls: Array<{
  selector: string
  component: Component
  extraProps?: Record<string, unknown>
}> = [
  { selector: 'input', component: AppInput },
  { selector: 'textarea', component: AppTextarea },
  {
    selector: 'select',
    component: AppSelect,
    extraProps: { options: [{ value: 'codex', label: 'Codex' }] },
  },
]

describe.each(controls)('$selector field composition', ({ selector, component, extraProps }) => {
  it('accepts AppField controlProps without attribute translation', () => {
    const wrapper = mount(AppField, {
      props: {
        id: selector + '-control',
        label: 'Control',
        hint: 'Choose carefully.',
        error: 'This value is invalid.',
        required: true,
      },
      slots: {
        default: ({ controlProps }: { controlProps: AppFieldControlProps }) =>
          h(component, {
            ...controlProps,
            ...extraProps,
            modelValue: '',
          }),
      },
    })
    const control = wrapper.get(selector)

    expect(control.attributes()).toMatchObject({
      id: selector + '-control',
      'aria-describedby': selector + '-control-hint ' + selector + '-control-error',
      'aria-invalid': 'true',
      'aria-required': 'true',
    })
  })
})
