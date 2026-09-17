import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import AppSegmentedControl from '../src/components/AppSegmentedControl.vue'

const options = [
  { id: 'compact', label: 'Compact' },
  { id: 'comfortable', label: 'Comfortable' },
  { id: 'dense', label: 'Dense', disabled: true },
]

afterEach(() => {
  document.body.replaceChildren()
})

describe('AppSegmentedControl', () => {
  it('renders a named radiogroup with a single selected option', () => {
    const wrapper = mount(AppSegmentedControl, {
      attachTo: document.body,
      props: { modelValue: 'compact', options, ariaLabel: 'Density' },
    })
    const radios = wrapper.findAll('[role="radio"]')

    expect(wrapper.attributes('role')).toBe('radiogroup')
    expect(wrapper.attributes('aria-label')).toBe('Density')
    expect(radios).toHaveLength(3)
    expect(radios[0]?.attributes()).toMatchObject({
      'aria-checked': 'true',
      tabindex: '0',
    })
    expect(radios[2]?.attributes('disabled')).toBeDefined()
  })

  it('moves through enabled options and focuses the selected radio', async () => {
    const wrapper = mount(AppSegmentedControl, {
      attachTo: document.body,
      props: { modelValue: 'compact', options, ariaLabel: 'Density' },
    })

    await wrapper.findAll('[role="radio"]')[0]?.trigger('keydown', { key: 'ArrowRight' })
    await nextTick()

    expect(wrapper.emitted('update:modelValue')).toEqual([['comfortable']])
    expect(document.activeElement).toBe(wrapper.findAll('[role="radio"]')[1]?.element)
  })
})
