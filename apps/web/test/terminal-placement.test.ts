import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

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
