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

  it('marks the selected card with the ring the reference draws, not a badge', () => {
    const selected = mount(ThemePreviewCard, {
      props: { value: 'dark', label: 'Dark', selected: true },
    })
    const unselected = mount(ThemePreviewCard, {
      props: { value: 'light', label: 'Light', selected: false },
    })

    // The reference's selected card is the one with the ring around the
    // miniature; a tick in the corner is a second, louder way of saying what
    // the ring already said, and it covers the artwork it sits on.
    expect(selected.get('[data-preview-frame]').attributes('data-preview-selected')).toBe('true')
    expect(selected.find('[data-preview-check]').exists()).toBe(false)
    expect(unselected.get('[data-preview-frame]').attributes('data-preview-selected')).toBeUndefined()
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
