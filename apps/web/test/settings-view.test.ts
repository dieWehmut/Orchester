import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import SettingsView from '../src/views/SettingsView.vue'
import { resetAppearanceForTests } from '@orchester/design'

const originalMatchMedia = window.matchMedia

/** jsdom answers every media query false and never emits a change. */
function stubSystemPreference(options: { dark?: boolean; reducedMotion?: boolean }): void {
  window.matchMedia = ((query: string): MediaQueryList =>
    ({
      matches: query.includes('prefers-reduced-motion')
        ? options.reducedMotion === true
        : query.includes('light')
          ? options.dark !== true
          : false,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    }) as MediaQueryList) as typeof window.matchMedia
}

beforeEach(() => {
  localStorage.clear()
  resetAppearanceForTests()
  for (const attribute of ['data-theme', 'data-color-scheme', 'data-intensity', 'data-reduced-motion']) {
    document.documentElement.removeAttribute(attribute)
  }
  stubSystemPreference({ dark: true, reducedMotion: false })
})

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

describe('SettingsView', () => {
  it('renders a sectioned settings surface with a navigation list', () => {
    const wrapper = mount(SettingsView)

    const sections = wrapper
      .findAll('[data-settings-section]')
      .map((node) => node.attributes('data-settings-section'))

    expect(sections).toEqual([
      'general',
      'appearance',
      'notifications',
      'import',
      'profile',
      'providers',
      'about',
    ])
    expect(wrapper.get('[data-settings-nav]')).toBeTruthy()
    expect(wrapper.get('[data-settings-nav]').text()).toContain('Appearance')
  })

  it('opens on appearance, the section the shell sends the user to', () => {
    const wrapper = mount(SettingsView)

    expect(wrapper.get('[data-settings-section="appearance"]').attributes('aria-selected')).toBe('true')
  })

  it('keeps every section reachable from the navigation list', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-settings-nav-link="providers"]').trigger('click')

    expect(wrapper.get('[data-settings-section="providers"]').attributes('aria-selected')).toBe('true')
    expect(wrapper.get('[data-settings-section="appearance"]').attributes('aria-selected')).toBe('false')
  })

  it('offers the three theme cards the reference surface shows', () => {
    const wrapper = mount(SettingsView)
    const cards = wrapper.findAll('[data-theme-option]').map((node) => node.attributes('data-theme-option'))

    expect(cards).toEqual(['system', 'light', 'dark'])
  })

  it('marks the active theme card as checked', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-theme-option="dark"]').trigger('click')

    expect(wrapper.get('[data-theme-option="dark"]').attributes('aria-checked')).toBe('true')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  it('stores an explicit system choice rather than clearing the key', async () => {
    const wrapper = mount(SettingsView)
    await wrapper.get('[data-theme-option="light"]').trigger('click')
    expect(localStorage.getItem('orchester:theme')).toBe('light')

    await wrapper.get('[data-theme-option="system"]').trigger('click')

    // "system" is a real preference and is stored as one; the resolved theme
    // follows the OS, which the stub reports as dark.
    expect(localStorage.getItem('orchester:theme')).toBe('system')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  it('selects the accent scheme from the appearance table', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-appearance-field="scheme"] select').setValue('teal')

    expect(document.documentElement.getAttribute('data-color-scheme')).toBe('teal')
    expect(localStorage.getItem('orchester:color-scheme')).toBe('teal')
  })

  it('selects the intensity axis from the appearance table', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-appearance-field="intensity"] select').setValue('calm')

    expect(document.documentElement.getAttribute('data-intensity')).toBe('calm')
    expect(localStorage.getItem('orchester:intensity')).toBe('calm')
  })

  it('toggles reduced motion between follow-the-OS and explicit', async () => {
    const wrapper = mount(SettingsView)
    const toggle = wrapper.get('[data-appearance-field="reduced-motion"] [role="switch"]')

    expect(toggle.attributes('aria-checked')).toBe('false')

    await toggle.trigger('click')
    expect(document.documentElement.getAttribute('data-reduced-motion')).toBe('true')

    await toggle.trigger('click')
    expect(document.documentElement.getAttribute('data-reduced-motion')).toBe('false')
  })

  it('reports the surface the host announced', () => {
    const wrapper = mount(SettingsView)

    expect(wrapper.get('[data-appearance-field="surface"]').text()).toContain('web')
  })

  it('selects the interface and content faces from the appearance table', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-appearance-field="ui-font"] select').setValue('serif')
    expect(document.documentElement.getAttribute('data-ui-font')).toBe('serif')
    expect(localStorage.getItem('orchester:ui-font')).toBe('serif')

    await wrapper.get('[data-appearance-field="content-font"] select').setValue('ui')
    expect(document.documentElement.getAttribute('data-content-font')).toBe('ui')
    expect(localStorage.getItem('orchester:content-font')).toBe('ui')
  })

  it('sets a weight per face, because the UI and content faces differ', async () => {
    const wrapper = mount(SettingsView)

    const uiRow = wrapper.get('[data-appearance-field="ui-font"]')
    const contentRow = wrapper.get('[data-appearance-field="content-font"]')

    await uiRow.findAll('select')[1].setValue('medium')
    expect(document.documentElement.getAttribute('data-ui-font-weight')).toBe('medium')
    expect(localStorage.getItem('orchester:ui-font-weight')).toBe('medium')

    // The content face is still on its own default: one row, one axis.
    expect(document.documentElement.getAttribute('data-content-font-weight')).toBe('regular')

    await contentRow.findAll('select')[1].setValue('medium')
    expect(document.documentElement.getAttribute('data-content-font-weight')).toBe('medium')
  })

  it('offers the translucent rail only where a window can composite behind it', async () => {
    const wrapper = mount(SettingsView)
    const toggle = wrapper.get('[data-appearance-field="rail-appearance"] [role="switch"]')

    // The browser surface has nothing behind the window to see through.
    expect(toggle.attributes('disabled')).toBeDefined()

    await toggle.trigger('click')
    expect(localStorage.getItem('orchester:rail-appearance')).toBeNull()
  })

  it('reports the background and foreground the theme resolves to', () => {
    const wrapper = mount(SettingsView)

    // Read-only readouts, not pickers: the two colours are what the chosen
    // theme resolves to, and a hex box here would be a second theme editor.
    const background = wrapper.get('[data-appearance-field="background"]')
    const foreground = wrapper.get('[data-appearance-field="foreground"]')

    expect(background.find('[data-color-readout]').exists()).toBe(true)
    expect(foreground.find('[data-color-readout]').exists()).toBe(true)
    expect(background.find('select').exists()).toBe(false)
    expect(foreground.find('select').exists()).toBe(false)
  })

  it('renders a live code preview that shows the active accents', () => {
    const wrapper = mount(SettingsView)

    const preview = wrapper.get('[data-code-preview]')
    expect(preview.text()).toContain('themePreview')
    expect(preview.text()).toContain('accent')
  })

  it('paints the preview accent from the active scheme', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-appearance-field="scheme"] select').setValue('teal')

    expect(wrapper.get('[data-code-preview]').text()).toContain('#4fbfad')
  })
})
