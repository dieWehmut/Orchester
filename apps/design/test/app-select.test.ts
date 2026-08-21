import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppSelect from '../src/components/AppSelect.vue'

const options = [
  { value: 'claude', label: 'Claude' },
  { value: 'codex', label: 'Codex' },
  { value: 'legacy', label: 'Legacy model', disabled: true },
]

describe('AppSelect', () => {
  it('renders a controlled native select and emits the selected value', async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: 'claude',
        id: 'model',
        options,
        describedBy: 'model-hint',
        required: true,
      },
    })
    const select = wrapper.get('select')

    expect(select.element.value).toBe('claude')
    expect(select.attributes()).toMatchObject({
      id: 'model',
      'aria-describedby': 'model-hint',
      'aria-required': 'true',
    })
    expect(wrapper.findAll('option')).toHaveLength(3)
    expect(wrapper.findAll('option')[2]?.attributes('disabled')).toBeDefined()

    await select.setValue('codex')

    expect(wrapper.emitted('update:modelValue')).toEqual([['codex']])
  })

  it('adds a disabled placeholder without selecting a hidden value', () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: '',
        options,
        placeholder: 'Choose a model',
        invalid: true,
        disabled: true,
      },
    })
    const select = wrapper.get('select')
    const placeholder = wrapper.get('option')

    expect(placeholder.text()).toBe('Choose a model')
    expect(placeholder.attributes('disabled')).toBeDefined()
    expect(select.attributes('aria-invalid')).toBe('true')
    expect(select.attributes('disabled')).toBeDefined()
  })
})
