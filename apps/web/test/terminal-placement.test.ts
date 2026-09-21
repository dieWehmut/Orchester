import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import InspectorDock from '../src/components/layout/InspectorDock.vue'
import SettingsView from '../src/views/SettingsView.vue'
import {
  TERMINAL_PLACEMENT_STORAGE_KEY,
  readTerminalPlacement,
  writeTerminalPlacement,
} from '../src/components/layout/terminal-placement'

/**
 * The terminal's home, section 4.7 of the design spec.
 *
 * Section 4.7 gives the Terminal tab to the inspector and notes that a
 * preference chooses whether terminal tabs open in the inspector or the bottom
 * panel. Section 4.8 gives the bottom panel the same three surfaces. A user who
 * moves the terminal has said where terminals belong for them, so the choice is
 * stored rather than asked again on every mount.
 */

beforeEach(() => {
  localStorage.clear()
})

describe('terminal placement preference', () => {
  it('opens in the bottom panel until the reader moves it', () => {
    // The bottom panel is the panel's own first surface and the terminal's
    // traditional home, so it is the state a reader meets before choosing.
    expect(readTerminalPlacement()).toBe('bottom')
  })

  it('round-trips the inspector through storage', () => {
    writeTerminalPlacement('inspector')

    expect(readTerminalPlacement()).toBe('inspector')
    expect(localStorage.getItem(TERMINAL_PLACEMENT_STORAGE_KEY)).toBe('inspector')
  })

  it('treats a value that is not a placement as no value at all', () => {
    localStorage.setItem(TERMINAL_PLACEMENT_STORAGE_KEY, 'sidebar')

    expect(readTerminalPlacement()).toBe('bottom')
  })

  it('moves the terminal back to the bottom panel', () => {
    writeTerminalPlacement('inspector')
    writeTerminalPlacement('bottom')

    expect(readTerminalPlacement()).toBe('bottom')
  })
})

describe('the surfaces the preference moves', () => {
  it('offers the terminal in the dock only when the preference puts it there', async () => {
    const away = mount(InspectorDock)
    expect(away.findAll('[role="tab"]').map((tab) => tab.text())).toEqual([
      'Context',
      'Approvals',
      'Review',
    ])

    const here = mount(InspectorDock, { props: { terminal: true } })
    expect(here.findAll('[role="tab"]').map((tab) => tab.text())).toContain('Terminal')
  })

  it('takes the terminal off the bottom panel once it lives in the inspector', () => {
    // A preference that moves the terminal has to take it off the panel, or
    // the reader gets two terminals and a tab that opens nothing they asked
    // for.
    const view = readFileSync(
      resolve(process.cwd(), 'src', 'views', 'WorkspaceView.vue'),
      'utf8',
    )

    expect(view).toContain('terminalPlacement')
    expect(view).toContain(":terminal=\"terminalPlacement === 'inspector'\"")
  })

  it('renders the control that changes the preference', async () => {
    const wrapper = mount(SettingsView)

    const control = wrapper.get('[data-settings-field="terminal-placement"]')
    expect(control.text()).toContain('Terminal')

    await control.findAll('[role="radio"]')[1]?.trigger('click')
    expect(readTerminalPlacement()).toBe('inspector')
  })
})
