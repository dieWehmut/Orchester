import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import TabStrip from '../src/components/layout/TabStrip.vue'
import type { ShellTab } from '../src/components/layout/tab-strip'

/**
 * The unified tab strip, region B of the shell.
 *
 * Section 4.2 makes it the one strip for open tasks, terminals, diffs and
 * agents, so a tab is identified by what it opens rather than by which pane
 * drew it. It has to cycle and select by keyboard, close on a middle click and
 * on the window's own close chord, reorder by drag, and stay usable when it
 * overflows - a strip that silently hides the tab a user cannot see is the
 * failure this clause exists for. Remembering the tabs across a restart is
 * U2-05's job, not this one's.
 */

const TABS: readonly ShellTab[] = [
  { id: 'run:1', kind: 'run', label: 'Inspect the runtime' },
  { id: 'terminal:1', kind: 'terminal', label: 'Terminal' },
  { id: 'diff:1', kind: 'diff', label: 'Changes' },
]

function mountStrip(props: Partial<{ tabs: readonly ShellTab[]; activeId: string }> = {}) {
  return mount(TabStrip, {
    props: {
      tabs: props.tabs ?? TABS,
      activeId: props.activeId ?? 'run:1',
      label: 'Open tabs',
    },
  })
}

describe('TabStrip', () => {
  it('is the strip the workspace mounts, and it keeps the run tab first', () => {
    // Region B exists for the product only if the product is drawn inside it.
    const view = readFileSync(resolve(process.cwd(), 'src', 'views', 'WorkspaceView.vue'), 'utf8')

    expect(view).toContain("from '../components/layout/TabStrip.vue'")
    expect(view).toContain('<TabStrip')
    // The transcript cannot be collapsed, so the run is the strip's first tab.
    expect(view).toMatch(/const tabs: ShellTab\[\] = \[\{ id: 'run'/)
  })

  it('carries the region attributes the shell contract names', () => {
    const wrapper = mountStrip()

    const strip = wrapper.get('[data-tabstrip]')
    expect(strip.attributes('data-tabstrip-overflow')).toBe('false')
    expect(strip.attributes('style')).toContain('--tabstrip-height')
  })

  it('renders one tab per open surface with the active one marked', () => {
    const wrapper = mountStrip()

    const labels = wrapper.findAll('[data-tabstrip-label]').map((label) => label.text())
    expect(labels).toEqual(['Inspect the runtime', 'Terminal', 'Changes'])
    expect(wrapper.get('[data-tabstrip-tab="run:1"]').attributes('aria-selected')).toBe('true')
    expect(wrapper.get('[data-tabstrip-tab="terminal:1"]').attributes('aria-selected')).toBe('false')
  })

  it('selects a tab by click and by the number chord', async () => {
    const wrapper = mountStrip()

    await wrapper.get('[data-tabstrip-tab="diff:1"]').trigger('click')
    expect(wrapper.emitted('select')?.at(-1)).toEqual(['diff:1'])

    await wrapper.get('[role="tablist"]').trigger('keydown', { key: '2', ctrlKey: true })
    expect(wrapper.emitted('select')?.at(-1)).toEqual(['terminal:1'])
  })

  it('cycles with the tab chord from wherever the selection is', async () => {
    const forward = mountStrip()
    await forward.get('[role="tablist"]').trigger('keydown', { key: 'Tab', ctrlKey: true })
    expect(forward.emitted('select')?.at(-1)).toEqual(['terminal:1'])

    const backward = mountStrip({ activeId: 'terminal:1' })
    await backward
      .get('[role="tablist"]')
      .trigger('keydown', { key: 'Tab', ctrlKey: true, shiftKey: true })
    expect(backward.emitted('select')?.at(-1)).toEqual(['run:1'])
  })

  it('wraps the cycle at both ends rather than dead-ending', async () => {
    const wrapper = mountStrip({ activeId: 'diff:1' })
    const list = wrapper.get('[role="tablist"]')

    await list.trigger('keydown', { key: 'Tab', ctrlKey: true })
    expect(wrapper.emitted('select')?.at(-1)).toEqual(['run:1'])

    // Backwards from the last tab is the tab before it, not the last one again.
    await list.trigger('keydown', { key: 'Tab', ctrlKey: true, shiftKey: true })
    expect(wrapper.emitted('select')?.at(-1)).toEqual(['terminal:1'])

    // Backwards from the first wraps to the last.
    const atFirst = mountStrip()
    await atFirst
      .get('[role="tablist"]')
      .trigger('keydown', { key: 'Tab', ctrlKey: true, shiftKey: true })
    expect(atFirst.emitted('select')?.at(-1)).toEqual(['diff:1'])
  })

  it('closes a tab on a middle click', async () => {
    const wrapper = mountStrip()

    await wrapper.get('[data-tabstrip-tab="terminal:1"]').trigger('auxclick', { button: 1 })
    expect(wrapper.emitted('close')?.at(-1)).toEqual(['terminal:1'])
  })

  it('leaves the window close chord to the shell, so one press closes one thing', async () => {
    // The strip answers the chords that move between its own tabs. Mod+W is not
    // one of them: the shell answers it, because once the last tab is in front
    // that same chord means the window. A second owner here would close the tab
    // and then the window for a single press.
    const wrapper = mountStrip()

    await wrapper.get('[role="tablist"]').trigger('keydown', { key: 'w', ctrlKey: true })

    expect(wrapper.emitted('close')).toBeUndefined()
  })

  it('reorders by dragging one tab onto another', async () => {
    const wrapper = mountStrip()
    const from = wrapper.get('[data-tabstrip-tab="diff:1"]')
    const onto = wrapper.get('[data-tabstrip-tab="run:1"]')

    await from.trigger('dragstart')
    await onto.trigger('drop')

    expect(wrapper.emitted('reorder')?.at(-1)).toEqual([{ from: 'diff:1', to: 'run:1' }])
  })

  it('names every tab and every close control', () => {
    const wrapper = mountStrip()

    for (const close of wrapper.findAll('[data-tabstrip-close]')) {
      expect((close.attributes('aria-label') ?? '').length).toBeGreaterThan(0)
    }
    const list = wrapper.get('[role="tablist"]')
    expect((list.attributes('aria-label') ?? '').length).toBeGreaterThan(0)
  })

  it('reports overflow so the edge fades have something to follow', async () => {
    const wrapper = mountStrip()

    // The strip measures itself: a scroll width past its client width is what
    // "the tabs do not fit" means, and it is the honest signal available
    // without laying the strip out for real.
    const element = wrapper.get('[role="tablist"]').element
    Object.defineProperty(element, 'scrollWidth', { value: 900, configurable: true })
    Object.defineProperty(element, 'clientWidth', { value: 400, configurable: true })
    await wrapper.get('[role="tablist"]').trigger('scroll')

    expect(wrapper.get('[data-tabstrip]').attributes('data-tabstrip-overflow')).toBe('true')
  })

  it('keeps the transcript reachable when the last tab closes', async () => {
    // Closing the only tab is not "close everything": the transcript is the
    // product and cannot be collapsed, so the strip reports the close and stays.
    const wrapper = mountStrip({ tabs: [TABS[0]!], activeId: 'run:1' })

    await wrapper.get('[data-tabstrip-close]').trigger('click')
    expect(wrapper.emitted('close')?.at(-1)).toEqual(['run:1'])
    // `get` would have thrown if the strip had gone, so reaching it is the
    // assertion that the last close did not empty the region.
    expect(wrapper.get('[data-tabstrip]').isVisible()).toBe(true)
  })
})
