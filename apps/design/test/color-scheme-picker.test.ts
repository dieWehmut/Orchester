import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import {
  COLOR_SCHEME_STORAGE_KEY,
  ColorSchemePicker,
  resetAppearanceForTests,
} from '../src'

beforeEach(() => {
  localStorage.clear()
  resetAppearanceForTests()
  document.documentElement.removeAttribute('data-color-scheme')
})

afterEach(() => {
  document.body.replaceChildren()
})

describe('ColorSchemePicker keyboard navigation', () => {
  it('uses roving tabindex with only the selected swatch tabbable', () => {
    const wrapper = mount(ColorSchemePicker, { attachTo: document.body })
    const swatches = wrapper.findAll('[role="radio"]')

    expect(swatches[0]?.attributes('tabindex')).toBe('0')
    expect(swatches.slice(1).every((swatch) => swatch.attributes('tabindex') === '-1')).toBe(true)
  })

  it('selects and focuses the next swatch with ArrowRight', async () => {
    const wrapper = mount(ColorSchemePicker, { attachTo: document.body })
    const swatches = wrapper.findAll('[role="radio"]')

    await swatches[0]?.trigger('keydown', { key: 'ArrowRight' })

    expect(swatches[1]?.attributes('aria-checked')).toBe('true')
    expect(document.activeElement).toBe(swatches[1]?.element)
    expect(localStorage.getItem(COLOR_SCHEME_STORAGE_KEY)).toBe('violet')
  })

  it('wraps with Home and End without changing the focus contract', async () => {
    const wrapper = mount(ColorSchemePicker, { attachTo: document.body })
    const swatches = wrapper.findAll('[role="radio"]')

    await swatches[0]?.trigger('keydown', { key: 'End' })
    expect(document.activeElement).toBe(swatches[3]?.element)
    expect(swatches[3]?.attributes('aria-checked')).toBe('true')

    await swatches[3]?.trigger('keydown', { key: 'Home' })
    expect(document.activeElement).toBe(swatches[0]?.element)
    expect(swatches[0]?.attributes('aria-checked')).toBe('true')
  })
})
