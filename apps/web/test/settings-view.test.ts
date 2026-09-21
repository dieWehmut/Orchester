import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { resetPetVisibilityForTests } from '../src/features/pet'
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
      'keybindings',
      'pet',
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

  it('carries a return-to-app row above the navigation, as the reference does', () => {
    const wrapper = mount(SettingsView)

    // The reference puts the way back at the head of its own settings nav
    // rather than leaving the reader to the browser's history.
    const back = wrapper.get('[data-settings-back]')
    expect(back.attributes('data-settings-back')).toBe('workspace')
    expect(back.text().length).toBeGreaterThan(0)
  })

  it('prints the hex each colour readout resolves to, beside its swatch', () => {
    const wrapper = mount(SettingsView)

    // The reference shows #FFFFFF and #1A1C1F next to the swatches, so the
    // value is readable rather than only visible.
    const background = wrapper.get('[data-appearance-field="background"]')
    const foreground = wrapper.get('[data-appearance-field="foreground"]')

    expect(background.get('[data-color-hex]').text()).toMatch(/^#[0-9A-F]{6}$/)
    expect(foreground.get('[data-color-hex]').text()).toMatch(/^#[0-9A-F]{6}$/)
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

    const uiFaces = uiRow.findAll('select')
    const contentFaces = contentRow.findAll('select')
    expect(uiFaces).toHaveLength(2)
    expect(contentFaces).toHaveLength(2)

    await uiFaces[1]!.setValue('medium')
    expect(document.documentElement.getAttribute('data-ui-font-weight')).toBe('medium')
    expect(localStorage.getItem('orchester:ui-font-weight')).toBe('medium')

    // The content face is still on its own default: one row, one axis.
    expect(document.documentElement.getAttribute('data-content-font-weight')).toBe('regular')

    await contentFaces[1]!.setValue('medium')
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

  it('offers import, export and reset above the appearance table', () => {
    const wrapper = mount(SettingsView)
    const actions = wrapper.get('[data-settings-actions]')

    // Named by their action attribute rather than their label: the labels are
    // translated, and a test that pins the English ones would break the moment
    // the suite ran under another locale.
    expect(actions.findAll('button')).toHaveLength(3)
    for (const action of ['import', 'export', 'reset']) {
      expect(actions.get(`[data-action="${action}"]`).text().length).toBeGreaterThan(0)
    }
    expect(actions.get('[data-action="import"]').text()).toContain('Import')
    expect(actions.get('[data-action="export"]').text()).toContain('Export')
    expect(actions.get('[data-action="reset"]').text()).toContain('Reset')
  })

  it('resets every axis and forgets what was stored', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-appearance-field="ui-font"] select').setValue('serif')
    expect(localStorage.getItem('orchester:ui-font')).toBe('serif')

    await wrapper.get('[data-settings-actions] [data-action="reset"]').trigger('click')

    expect(localStorage.getItem('orchester:ui-font')).toBeNull()
    expect(document.documentElement.getAttribute('data-ui-font')).toBe('system')
  })

  it('reports a file that is not a profile instead of half-applying it', async () => {
    const wrapper = mount(SettingsView)
    const input = wrapper.get('input[type="file"]')
    const file = new File(['{"version":1,"colorScheme":"chartreuse"}'], 'broken.json', {
      type: 'application/json',
    })

    Object.defineProperty(input.element, 'files', { value: [file], configurable: true })
    // The handler reads the file, so the rejection lands a tick after the event.
    await input.trigger('change')
    await flushPromises()

    expect(wrapper.get('[role="alert"]').text().length).toBeGreaterThan(0)
    expect(document.documentElement.getAttribute('data-color-scheme')).not.toBe('chartreuse')
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

  it('toggles the ambient companion from the pet section', async () => {
    const wrapper = mount(SettingsView)

    await wrapper.get('[data-settings-nav-link="pet"]').trigger('click')
    const section = wrapper.get('[data-settings-section="pet"]')
    expect(section.attributes('aria-selected')).toBe('true')

    const toggle = section.get('[role="switch"]')
    expect(toggle.attributes('aria-checked')).toBe('true')

    await toggle.trigger('click')
    expect(section.get('[role="switch"]').attributes('aria-checked')).toBe('false')
    localStorage.clear()
    resetPetVisibilityForTests()
  })
})
