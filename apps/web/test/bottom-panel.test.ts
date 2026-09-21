import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import BottomPanel from '../src/components/layout/BottomPanel.vue'
import {
  BOTTOM_PANEL_STATE_STORAGE_KEY,
  readBottomPanelState,
  writeBottomPanelState,
} from '../src/components/layout/bottom-panel-state'

/**
 * The bottom panel, region I of the shell.
 *
 * Section 2 gives it three surfaces - a terminal, exec output and an audit log -
 * and makes it collapsible, with the state carried as an attribute so a
 * stylesheet can key off it. Section 2.1 adds that the panel is resizable
 * between its floor and 70vh, and that under 900 px it is a drawer rather than
 * a column of its own.
 */

function mountPanel() {
  return mount(BottomPanel, {
    props: {
      label: 'Bottom panel',
      tabs: [
        { id: 'terminal', label: 'Terminal' },
        { id: 'output', label: 'Output' },
        { id: 'audit', label: 'Audit log' },
      ],
    },
    slots: {
      terminal: '<p data-testid="terminal">$ pnpm test</p>',
      output: '<p data-testid="output">exec</p>',
      audit: '<p data-testid="audit">audit</p>',
    },
  })
}

beforeEach(() => {
  localStorage.clear()
})

describe('BottomPanel', () => {
  it('is the panel the workspace mounts, not a second copy of it', () => {
    // Region I only exists for the product if the product is drawn inside it.
    const view = readFileSync(resolve(process.cwd(), 'src', 'views', 'WorkspaceView.vue'), 'utf8')

    expect(view).toContain("from '../components/layout/BottomPanel.vue'")
    expect(view).toContain('<BottomPanel')
    // The three surfaces the spec names are the three the view offers.
    for (const surface of ['terminal', 'output', 'audit']) {
      expect(view).toContain(`id: '${surface}'`)
    }
  })

  it('carries the region attributes the shell contract names', () => {
    const wrapper = mountPanel()

    const panel = wrapper.get('[data-bottom-panel]')
    expect(panel.attributes('data-bottom-panel-state')).toBe('collapsed')
  })

  it('starts collapsed and opens with its own tab mechanism', async () => {
    const wrapper = mountPanel()

    expect(wrapper.find('[data-bottom-panel-body]').exists()).toBe(false)

    await wrapper.get('[data-bottom-panel-toggle]').trigger('click')
    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('expanded')
    expect(wrapper.get('[data-bottom-panel-body]').text()).toContain('pnpm test')
  })

  it('switches between the three surfaces the spec lists', async () => {
    const wrapper = mountPanel()
    await wrapper.get('[data-bottom-panel-toggle]').trigger('click')

    const tabs = wrapper.findAll('[data-bottom-panel-tab]')
    expect(tabs.map((tab) => tab.text())).toEqual(['Terminal', 'Output', 'Audit log'])

    await tabs[2]!.trigger('click')
    expect(wrapper.get('[data-bottom-panel-body]').text()).toContain('audit')
    expect(wrapper.get('[data-bottom-panel-tab="audit"]').attributes('aria-selected')).toBe('true')
  })

  it('names the toggle and reports whether the panel is open', async () => {
    const wrapper = mountPanel()
    const toggle = wrapper.get('[data-bottom-panel-toggle]')

    expect((toggle.attributes('aria-label') ?? '').length).toBeGreaterThan(0)
    expect(toggle.attributes('aria-expanded')).toBe('false')

    await toggle.trigger('click')
    expect(wrapper.get('[data-bottom-panel-toggle]').attributes('aria-expanded')).toBe('true')
  })

  it('remembers the open state and the chosen surface', async () => {
    const first = mountPanel()
    await first.get('[data-bottom-panel-toggle]').trigger('click')
    await first.get('[data-bottom-panel-tab="output"]').trigger('click')

    expect(readBottomPanelState()).toEqual({ expanded: true, tab: 'output' })

    // A mount restores the stored state after its first render, so the panel
    // settles on the next tick rather than on the first paint.
    const second = mountPanel()
    await flushPromises()
    expect(second.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('expanded')
    expect(second.get('[data-bottom-panel-body]').text()).toContain('exec')
  })

  it('resizes between its floor and the share of the viewport the spec allows', async () => {
    const wrapper = mountPanel()
    await wrapper.get('[data-bottom-panel-toggle]').trigger('click')

    const handle = wrapper.get('[data-bottom-panel-resize]')
    expect(handle.attributes('role')).toBe('separator')

    await handle.trigger('keydown', { key: 'ArrowUp' })
    const grown = Number(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-height'))
    expect(grown).toBeGreaterThan(240)

    await handle.trigger('keydown', { key: 'Home' })
    expect(Number(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-height'))).toBe(160)

    await handle.trigger('keydown', { key: 'End' })
    const max = Number(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-height'))
    expect(max).toBeLessThanOrEqual(Math.round(window.innerHeight * 0.7))
  })

  it('tolerates a stored value that is not a panel state', () => {
    localStorage.setItem(BOTTOM_PANEL_STATE_STORAGE_KEY, '{"expanded":"yes"}')
    expect(readBottomPanelState()).toEqual({ expanded: false, tab: 'terminal' })

    writeBottomPanelState({ expanded: true, tab: 'audit' })
    expect(readBottomPanelState()).toEqual({ expanded: true, tab: 'audit' })
  })
})
