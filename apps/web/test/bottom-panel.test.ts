import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import BottomPanel from '../src/components/layout/BottomPanel.vue'
import {
  BOTTOM_PANEL_STATE_STORAGE_KEY,
  readBottomPanelState,
  writeBottomPanelState,
} from '../src/components/layout/bottom-panel-state'

/**
 * The bottom panel, region I of the shell.
 *
 * Section 2 gives it a terminal, exec output and an audit log, and makes it
 * collapsible with its own tab mechanism. It also holds the run's own surfaces
 * now that the shell has no right column, which is why the open state and the
 * chosen surface are props rather than its own state: the view opens the panel
 * on the approvals tab when an approval arrives, and a panel that kept its own
 * copy of "which surface" would fight it.
 *
 * The height stays internal, between the floor the spec states and the share of
 * the viewport it allows.
 */

function mountPanel(expanded = false, activeTab = 'terminal') {
  return mount(BottomPanel, {
    props: {
      label: 'Bottom panel',
      expanded,
      activeTab,
      tabs: [
        { id: 'context', label: 'Context' },
        { id: 'approvals', label: 'Approvals' },
        { id: 'terminal', label: 'Terminal' },
        { id: 'output', label: 'Output' },
        { id: 'audit', label: 'Audit log' },
      ],
    },
    slots: {
      context: '<p data-testid="context">agent</p>',
      terminal: '<p data-testid="terminal">$ pnpm test</p>',
      output: '<p data-testid="output">exec</p>',
      audit: '<p data-testid="audit">audit</p>',
    },
  })
}

const TABS = [
  { id: 'context', label: 'Context' },
  { id: 'approvals', label: 'Approvals' },
  { id: 'terminal', label: 'Terminal' },
  { id: 'output', label: 'Output' },
  { id: 'audit', label: 'Audit log' },
] as const

const SLOTS = {
  context: '<p data-testid="context">agent</p>',
  terminal: '<p data-testid="terminal">$ pnpm test</p>',
  output: '<p data-testid="output">exec</p>',
  audit: '<p data-testid="audit">audit</p>',
}

/**
 * The panel as its owner mounts it: the surface holds the state and the panel
 * reports what the reader asked for, which is what lets the view open it on the
 * approvals tab without a second copy of "which surface" existing.
 */
function mountControlled(expanded = false, activeTab = 'terminal') {
  const Host = defineComponent({
    setup() {
      const open = ref(expanded)
      const tab = ref(activeTab)
      return () =>
        h(BottomPanel, {
          label: 'Bottom panel',
          tabs: TABS,
          expanded: open.value,
          activeTab: tab.value,
          'onUpdate:expanded': (value: boolean) => (open.value = value),
          'onUpdate:activeTab': (value: string) => (tab.value = value),
        }, SLOTS)
    },
  })
  return mount(Host)
}

beforeEach(() => {
  localStorage.clear()
})

describe('BottomPanel', () => {
  it('is the panel the workspace mounts, not a second copy of it', () => {
    // Region I only exists for the product if the product is drawn inside it,
    // and the run's surfaces have to be among the panel's own tabs now that
    // there is no right column to hold them.
    const view = readFileSync(resolve(process.cwd(), 'src', 'views', 'WorkspaceView.vue'), 'utf8')

    expect(view).toContain("from '../components/layout/BottomPanel.vue'")
    expect(view).toContain('<BottomPanel')
    for (const surface of ['terminal', 'output', 'audit', 'context', 'approvals', 'changes']) {
      expect(view).toContain(`id: '${surface}'`)
    }
  })

  it('carries the region attributes the shell contract names', () => {
    const collapsed = mountPanel()
    const opened = mountPanel(true)

    expect(collapsed.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe(
      'collapsed',
    )
    expect(opened.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('expanded')
  })

  it('opens and closes by asking the surface that owns the state', async () => {
    const wrapper = mountControlled()

    expect(wrapper.find('[data-bottom-panel-body]').exists()).toBe(false)

    await wrapper.get('[data-bottom-panel-toggle]').trigger('click')

    // The panel reports the intent; it does not decide. A view that opens it on
    // an approval and a panel that closed itself on a click would disagree.
    expect(wrapper.findComponent(BottomPanel).emitted('update:expanded')).toEqual([[true]])
    expect(wrapper.get('[data-bottom-panel-body]').text()).toContain('pnpm test')

    await wrapper.get('[data-bottom-panel-toggle]').trigger('click')
    expect(wrapper.find('[data-bottom-panel-body]').exists()).toBe(false)
  })

  it('switches between the surfaces it was given', async () => {
    const wrapper = mountControlled(true)

    const tabs = wrapper.findAll('[data-bottom-panel-tab]')
    expect(tabs.map((tab) => tab.text())).toEqual([
      'Context',
      'Approvals',
      'Terminal',
      'Output',
      'Audit log',
    ])

    await tabs[4]!.trigger('click')

    expect(wrapper.findComponent(BottomPanel).emitted('update:activeTab')).toEqual([['audit']])
    expect(wrapper.get('[data-bottom-panel-body]').text()).toContain('audit')
    expect(wrapper.get('[data-bottom-panel-tab="audit"]').attributes('aria-selected')).toBe('true')
  })

  it('names the toggle and reports whether the panel is open', () => {
    const closed = mountPanel(false)
    const open = mountPanel(true)

    expect((closed.get('[data-bottom-panel-toggle]').attributes('aria-label') ?? '').length).toBeGreaterThan(0)
    expect(closed.get('[data-bottom-panel-toggle]').attributes('aria-expanded')).toBe('false')
    expect(open.get('[data-bottom-panel-toggle]').attributes('aria-expanded')).toBe('true')
  })

  it('resizes between its floor and the share of the viewport the spec allows', async () => {
    const wrapper = mountPanel(true)

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