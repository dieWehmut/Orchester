import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'
import PlanStrip from '../src/components/run/PlanStrip.vue'
import ReasoningDisclosure from '../src/components/run/ReasoningDisclosure.vue'
import RunPanel from '../src/components/run/RunPanel.vue'
import SettingsView from '../src/views/SettingsView.vue'
import ThreadBar from '../src/components/layout/ThreadBar.vue'
import ToolCallCard from '../src/components/run/ToolCallCard.vue'
import WindowChrome from '../src/components/layout/WindowChrome.vue'
import type { DesktopWindowController } from '../src/platform/desktop-window'

/**
 * The last two clauses of the accessibility contract in section 7: the
 * hit-target floor, and the names on icon-only and destructive controls.
 *
 * The floor is checked against the shared `--hit-target-min` token rather than
 * a second literal, so raising it for compact density raises it everywhere.
 * The caption buttons are checked against the titlebar token section 8 gives
 * them, because their width is the OS metric rather than the generic floor.
 *
 * Names are checked on the mounted control, not in the source: a slot that
 * fills a button with an icon and no text is exactly the case "every icon-only
 * control has an accessible name" exists for, and only the rendered element
 * knows which route the name takes.
 */

function source(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), 'src', relativePath), 'utf8')
}

function chromeController(): DesktopWindowController {
  return {
    enabled: true,
    minimize: async () => undefined,
    toggleMaximize: async () => undefined,
    close: async () => undefined,
    isMaximized: async () => false,
    listenMaximized: () => () => undefined,
  }
}

/** The accessible name a screen reader would compute for a rendered control. */
function accessibleName(element: Element): string {
  const labelled = element.getAttribute('aria-label')
  if (labelled?.trim()) return labelled.trim()
  const labelledBy = element.getAttribute('aria-labelledby')
  if (labelledBy) {
    const text = labelledBy
      .split(/\s+/)
      .map((id) => element.ownerDocument.getElementById(id)?.textContent?.trim() ?? '')
      .join(' ')
      .trim()
    if (text) return text
  }
  // A form control is named by the label that points at it, which is how
  // the composer names the prompt box, or by the one wrapping it.
  const id = element.getAttribute('id')
  if (id) {
    const root = element.getRootNode() as ParentNode
    const pointing = root.querySelector?.('label[for="' + id + '"]')
    const text = pointing?.textContent?.trim()
    if (text) return text
  }
  const wrapped = element.closest('label')?.textContent?.trim()
  if (wrapped) return wrapped
  const title = element.getAttribute('title')
  if (title?.trim()) return title.trim()
  return element.textContent?.trim() ?? ''
}

/** Every control a reader can reach in a mounted surface, in DOM order. */
function controlsOf(wrapper: { findAll: (selector: string) => { element: Element }[] }) {
  return wrapper.findAll('button, [role="button"], a[href], input:not([type="hidden"]), select, textarea')
}

describe('hit target contract in the workspace', () => {
  it('floors the caption buttons on the titlebar hit-target token', () => {
    // Section 8 gives the caption buttons the OS metric, and their inline size
    // was a hand-picked 22 px - under both that metric and the section 7 floor.
    const chrome = source('components/layout/WindowChrome.vue')
    const control = /\.window-chrome__control\s*\{[^}]*\}/s.exec(chrome)?.[0] ?? ''

    expect(control).toContain('var(--titlebar-hit-target')
    expect(control).not.toMatch(/inline-size:\s*\d+px/)
  })

  it('floors the toolbar and row controls the shell mounts', () => {
    // Each of these is a target on its own row, and each carried a box under
    // the floor before this clause was audited.
    const failures: string[] = []
    for (const [path, selector] of [
      ['components/layout/ThreadBar.vue', '.thread-bar__share'],
      ['components/run/PlanStrip.vue', '.plan-strip__toggle'],
      ['components/run/ReasoningDisclosure.vue', '.reasoning__toggle'],
      ['components/run/ToolCallCard.vue', '.tool-card__toggle'],
      ['components/run/RunPanel.vue', '.run-panel__to-bottom'],
      ['views/SettingsView.vue', '.settings-view__link'],
      ['components/settings/ShortcutEditor.vue', '.shortcut-editor__row'],
      ['components/sessions/SessionListItem.vue', '.session-list-item'],
      ['features/agent-presence/components/AgentFleetRow.vue', '.agent-fleet-row'],
      ['components/changes/ChangeInspector.vue', '.change-inspector__row'],
    ] as const) {
      const css = source(path)
      const body = new RegExp(`${selector.replace(/\./g, '\\.')}\\s*\\{[^}]*\\}`, 's').exec(css)?.[0] ?? ''
      // A `max()` floor is still a floor, so the token may sit inside one.
      const floored =
        /min-(?:block|inline)-size:[^;]*--hit-target-min/.test(body) ||
        /min-(?:block|inline)-size:[^;]*--density-row-height/.test(body)
      if (!floored) failures.push(`${path}: ${selector}`)
    }
    expect(failures).toEqual([])
  })
})

describe('accessible name contract in the workspace', () => {
  it('names every control the window chrome renders, icon-only included', () => {
    const wrapper = mount(WindowChrome, { props: { controller: chromeController() } })

    const unnamed = controlsOf(wrapper)
      .map((control) => accessibleName(control.element))
      .filter((name) => name.length === 0)
    expect(unnamed).toEqual([])
  })

  it('names every control the thread bar renders', () => {
    const wrapper = mount(ThreadBar, {
      props: { title: 'Thread', shareLabel: 'Share', moreLabel: 'More', panelLabel: 'Panel' },
    })

    const unnamed = controlsOf(wrapper)
      .map((control) => accessibleName(control.element))
      .filter((name) => name.length === 0)
    expect(unnamed).toEqual([])
  })

  it('names every control the run surface renders', () => {
    // The transcript's floating control and the disclosures live here, and the
    // composer is where a reader spends the most time.
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })

    const unnamed = controlsOf(wrapper)
      .map((control) => accessibleName(control.element))
      .filter((name) => name.length === 0)
    expect(unnamed).toEqual([])
  })

  it('names the plan and reasoning disclosures and the tool card toggle', () => {
    for (const wrapper of [
      mount(PlanStrip, { props: { todos: [{ text: 'Ship it', completed: false }] } }),
      mount(ReasoningDisclosure, { props: { text: 'why' } }),
      mount(ToolCallCard, {
        props: {
          item: { callId: 'call-1', name: 'shell', state: 'completed', detail: 'ls', summary: 'ls' },
        } as never,
      }),
    ]) {
      const unnamed = controlsOf(wrapper)
        .map((control) => accessibleName(control.element))
        .filter((name) => name.length === 0)
      expect(unnamed).toEqual([])
    }
  })

  it('names every control the settings surface renders', () => {
    const wrapper = mount(SettingsView)

    const unnamed = controlsOf(wrapper)
      .map((control) => accessibleName(control.element))
      .filter((name) => name.length === 0)
    expect(unnamed).toEqual([])
  })

  it('names the object of every destructive action', () => {
    // The clause is about the object, not the verb: "Reset" alone leaves the
    // reader guessing what they are about to reset, and the settings page has
    // two of them - the appearance profile and the shortcut bindings.
    const wrapper = mount(SettingsView)
    const actions = ['[data-action="reset"]', '[data-shortcut-reset]']

    for (const selector of actions) {
      const control = wrapper.find(selector)
      if (!control.exists()) continue
      const name = accessibleName(control.element)
      expect(name.length, selector).toBeGreaterThan(0)
      // A bare verb is not a name for an object.
      expect(['Reset', 'Reset all'], selector).not.toContain(name)
    }
  })
})