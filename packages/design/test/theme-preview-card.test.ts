import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import { ThemePreviewCard, resetAppearanceForTests } from '../src'

beforeEach(() => {
  localStorage.clear()
  resetAppearanceForTests()
  document.documentElement.removeAttribute('data-theme')
})

describe('ThemePreviewCard', () => {
  it('renders the three panes the reference card miniature shows', () => {
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'light', label: 'Light', selected: false },
    })

    expect(wrapper.get('[data-preview-pane="rail"]')).toBeTruthy()
    expect(wrapper.get('[data-preview-pane="header"]')).toBeTruthy()
    expect(wrapper.get('[data-preview-pane="transcript"]')).toBeTruthy()
  })

  it('is a radio in a group, so arrow keys move between the cards', () => {
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'system', label: 'System', selected: true },
    })

    expect(wrapper.attributes('role')).toBe('radio')
    expect(wrapper.attributes('aria-checked')).toBe('true')
    expect(wrapper.attributes('tabindex')).toBe('0')
  })

  it('leaves the unselected card out of the tab order', () => {
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'dark', label: 'Dark', selected: false },
    })

    expect(wrapper.attributes('tabindex')).toBe('-1')
  })

  it('emits the value it stands for when clicked', async () => {
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'dark', label: 'Dark', selected: false },
    })

    await wrapper.trigger('click')

    expect(wrapper.emitted('select')).toEqual([['dark']])
  })

  it('draws a check on the selected card rather than only a border', () => {
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'dark', label: 'Dark', selected: true },
    })

    expect(wrapper.get('[data-preview-check]')).toBeTruthy()
  })

  it('paints each pane from the tokens of the theme it previews', () => {
    // The miniature must not inherit the live theme, or the "light" card would
    // render dark on a dark desktop and the choice would be unreadable.
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'light', label: 'Light', selected: false },
    })

    expect(wrapper.attributes('data-preview-theme')).toBe('light')
  })

  it('shows the system card as a split of both themes', () => {
    const wrapper = mount(ThemePreviewCard, {
      props: { value: 'system', label: 'System', selected: false },
    })

    expect(wrapper.attributes('data-preview-theme')).toBe('system')
    expect(wrapper.get('[data-preview-pane="rail"]').attributes('data-preview-side')).toBe('light')
  })
})
