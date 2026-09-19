import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ShortcutEditor from '../src/components/settings/ShortcutEditor.vue'
import { createShortcutRegistry, type ShortcutDefinition } from '../src/shortcuts/registry'

const shortcuts: ShortcutDefinition[] = [
  { id: 'palette.open', label: 'Open the command palette', group: 'Composer', keys: ['Mod', 'K'] },
  { id: 'inspector.toggle', label: 'Toggle the inspector', group: 'Layout', keys: ['Mod', 'B'] },
]

function mountEditor(registry = createShortcutRegistry()) {
  for (const shortcut of shortcuts) registry.register(shortcut)
  const wrapper = mount(ShortcutEditor, {
    props: { registry, platform: 'windows' },
  })
  return { registry, wrapper }
}

/**
 * The shortcut editor.
 *
 * It has to read the same registry the components register into: a hand-written
 * list in the editor would drift the first time a component changed its keys,
 * and the reader would learn a shortcut that does nothing.
 */
describe('ShortcutEditor', () => {
  it('lists every registered shortcut, grouped', () => {
    const { wrapper } = mountEditor()

    const rows = wrapper.findAll('[data-shortcut-row]')
    expect(rows.map((row) => row.attributes('data-shortcut-row'))).toEqual([
      'palette.open',
      'inspector.toggle',
    ])
    expect(wrapper.text()).toContain('Open the command palette')
    expect(wrapper.findAll('[data-shortcut-group]').map((node) => node.attributes('data-shortcut-group'))).toEqual([
      'Composer',
      'Layout',
    ])
  })

  it('shows the keys under the platform name the reader uses', () => {
    const { wrapper } = mountEditor()

    expect(wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-keys]').text()).toBe('Ctrl+K')
  })

  it("shows the platform's own modifier when mounted on a Mac", () => {
    const registry = createShortcutRegistry()
    for (const shortcut of shortcuts) registry.register(shortcut)
    const wrapper = mount(ShortcutEditor, { props: { registry, platform: 'macos' } })

    expect(wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-keys]').text()).toBe('Cmd+K')
  })

  it('searches the list by what the shortcut does', async () => {
    const { wrapper } = mountEditor()

    await wrapper.get('[data-shortcut-search]').setValue('inspector')

    expect(wrapper.findAll('[data-shortcut-row]').map((row) => row.attributes('data-shortcut-row'))).toEqual([
      'inspector.toggle',
    ])
  })

  it('says when a search matched nothing', async () => {
    const { wrapper } = mountEditor()

    await wrapper.get('[data-shortcut-search]').setValue('kubernetes')

    expect(wrapper.findAll('[data-shortcut-row]')).toHaveLength(0)
    expect(wrapper.get('[data-shortcut-empty]').text().length).toBeGreaterThan(0)
  })

  it('rebinds a shortcut by capturing the next chord', async () => {
    const { registry, wrapper } = mountEditor()

    await wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-record]').trigger('click')
    const capture = wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-capture]')
    await capture.trigger('keydown', { key: 'J', ctrlKey: true })

    expect(registry.effectiveKeys('palette.open')).toEqual(['Mod', 'J'])
    expect(wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-keys]').text()).toBe('Ctrl+J')
  })

  it('shows the shortcut a rebinding replaced, not the shipped one', async () => {
    const { registry, wrapper } = mountEditor()
    registry.rebind('palette.open', ['Mod', 'P'])
    await wrapper.vm.$nextTick()

    expect(wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-keys]').text()).toBe('Ctrl+P')
  })

  it('resets every shortcut to the shipped default', async () => {
    const { registry, wrapper } = mountEditor()
    registry.rebind('palette.open', ['Mod', 'J'])
    await wrapper.vm.$nextTick()
    expect(wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-keys]').text()).toBe('Ctrl+J')

    await wrapper.get('[data-shortcut-reset]').trigger('click')

    expect(registry.effectiveKeys('palette.open')).toEqual(['Mod', 'K'])
    expect(wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-keys]').text()).toBe('Ctrl+K')
  })

  it('reports a chord that is already taken instead of stealing it', async () => {
    const { registry, wrapper } = mountEditor()

    await wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-record]').trigger('click')
    const capture = wrapper.get('[data-shortcut-row="palette.open"] [data-shortcut-capture]')
    await capture.trigger('keydown', { key: 'B', ctrlKey: true })

    // The other shortcut keeps its keys and the reader is told why.
    expect(registry.effectiveKeys('inspector.toggle')).toEqual(['Mod', 'B'])
    expect(wrapper.get('[data-shortcut-error]').text().length).toBeGreaterThan(0)
  })
})
