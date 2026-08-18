import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import {
  AppBadge,
  AppButton,
  COLOR_SCHEME_ATTRIBUTE,
  ColorSchemePicker,
  IconButton,
  Spinner,
  StatusDot,
  THEME_ATTRIBUTE,
  ThemeToggle,
  resetAppearanceForTests,
} from '../src'

beforeEach(() => {
  localStorage.clear()
  document.documentElement.removeAttribute(THEME_ATTRIBUTE)
  document.documentElement.removeAttribute(COLOR_SCHEME_ATTRIBUTE)
  resetAppearanceForTests()
})

afterEach(() => {
  resetAppearanceForTests()
})

describe('AppButton', () => {
  it('renders its slot and names the variant in a class', () => {
    const wrapper = mount(AppButton, {
      props: { variant: 'danger', size: 'sm' },
      slots: { default: 'Cancel run' },
    })

    expect(wrapper.text()).toBe('Cancel run')
    expect(wrapper.classes()).toContain('app-button--danger')
    expect(wrapper.classes()).toContain('app-button--sm')
  })

  it('stays focusable while busy', () => {
    // The whole reason `busy` is a separate prop: disabling a button mid-action
    // moves focus to <body> and loses a keyboard user's place in the page.
    const wrapper = mount(AppButton, { props: { busy: true } })

    expect(wrapper.attributes('aria-busy')).toBe('true')
    expect(wrapper.attributes('disabled')).toBeUndefined()
  })

  it('reports idle as aria-busy="false", which is what ARIA defaults to', () => {
    expect(mount(AppButton).attributes('aria-busy')).toBe('false')
  })

  it('emits click', async () => {
    const wrapper = mount(AppButton)

    await wrapper.trigger('click')

    expect(wrapper.emitted('click')).toHaveLength(1)
  })
})

describe('IconButton', () => {
  it('exposes the label as the accessible name and as a tooltip', () => {
    const wrapper = mount(IconButton, { props: { label: 'Copy transcript' } })

    expect(wrapper.attributes('aria-label')).toBe('Copy transcript')
    expect(wrapper.attributes('title')).toBe('Copy transcript')
  })

  it('reports toggle state through aria-pressed', () => {
    expect(mount(IconButton, { props: { label: 'Pin' } }).attributes('aria-pressed')).toBe('false')
    expect(
      mount(IconButton, { props: { label: 'Pin', active: true } }).attributes('aria-pressed'),
    ).toBe('true')
  })
})

describe('AppBadge', () => {
  it('names the meaning rather than the colour', () => {
    const wrapper = mount(AppBadge, { props: { tone: 'warning' }, slots: { default: 'waiting' } })

    expect(wrapper.classes()).toContain('app-badge--warning')
    expect(wrapper.text()).toBe('waiting')
  })
})

describe('StatusDot', () => {
  it('carries the status in text, not only in colour', () => {
    const wrapper = mount(StatusDot, { props: { status: 'error', label: 'Failed' } })

    expect(wrapper.attributes('role')).toBe('img')
    expect(wrapper.attributes('aria-label')).toBe('Failed')
  })

  it('pulses only while running', () => {
    const running = mount(StatusDot, { props: { status: 'running', label: 'Running' } })
    const waiting = mount(StatusDot, { props: { status: 'waiting', label: 'Waiting' } })

    expect(running.classes()).toContain('status-dot--pulse')
    expect(waiting.classes()).not.toContain('status-dot--pulse')
  })
})

describe('Spinner', () => {
  it('announces itself as a status region and sizes both axes', () => {
    const wrapper = mount(Spinner, { props: { label: 'Starting agent', size: 24 } })

    expect(wrapper.attributes('role')).toBe('status')
    expect(wrapper.attributes('aria-label')).toBe('Starting agent')
    expect(wrapper.attributes('style')).toContain('width: 24px')
    expect(wrapper.attributes('style')).toContain('height: 24px')
  })
})

describe('ThemeToggle', () => {
  it('labels the destination, not the current state, and switches on click', async () => {
    const wrapper = mount(ThemeToggle, {
      props: { labelDark: 'Zu hellem Thema wechseln', labelLight: 'Zu dunklem Thema wechseln' },
    })

    expect(wrapper.attributes('aria-label')).toBe('Zu hellem Thema wechseln')

    await wrapper.trigger('click')

    expect(document.documentElement.getAttribute(THEME_ATTRIBUTE)).toBe('light')
    expect(wrapper.attributes('aria-label')).toBe('Zu dunklem Thema wechseln')
  })
})

describe('ColorSchemePicker', () => {
  it('is a radio group with one swatch per scheme', () => {
    const wrapper = mount(ColorSchemePicker)
    const swatches = wrapper.findAll('[role="radio"]')

    expect(wrapper.attributes('role')).toBe('radiogroup')
    expect(swatches).toHaveLength(4)
    expect(swatches.filter((swatch) => swatch.attributes('aria-checked') === 'true')).toHaveLength(1)
  })

  it('resolves labels through the caller so the package owns no copy', () => {
    const wrapper = mount(ColorSchemePicker, {
      props: { label: (key: string) => `translated:${key}` },
    })

    expect(wrapper.findAll('[role="radio"]')[0]?.attributes('aria-label')).toBe(
      'translated:colorScheme.amber',
    )
  })

  it('selects the clicked scheme', async () => {
    const wrapper = mount(ColorSchemePicker)

    await wrapper.findAll('[role="radio"]')[2]?.trigger('click')

    expect(document.documentElement.getAttribute(COLOR_SCHEME_ATTRIBUTE)).toBe('teal')
    expect(wrapper.findAll('[role="radio"]')[2]?.attributes('aria-checked')).toBe('true')
  })
})
