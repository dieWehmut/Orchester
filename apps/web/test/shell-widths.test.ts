import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import AppShell from '../src/components/layout/AppShell.vue'
import {
  RAIL_WIDTH_STORAGE_KEY,
  clampRailWidth,
  readShellWidths,
  writeShellWidths,
} from '../src/components/layout/shell-widths'

/**
 * The shell's resizable clamp, section 2.1 of the design spec.
 *
 * The rail is the surface a user tunes once and then expects to stay tuned, so
 * it carries a clamp rather than a fixed width and remembers what it was set to.
 * The clamp is the rule the spec states: the rail must leave the transcript its
 * 360 px, whatever the pointer asks for.
 *
 * The inspector's own clamp is gone with the column: the shell draws two
 * regions, and the run's surfaces are drawn in the bottom panel inside the
 * transcript's.
 */

function mountShell() {
  return mount(AppShell, {
    props: { sessionsTitle: 'Sessions' },
    slots: {
      sessions: '<p>Sessions</p>',
      default: '<p>Transcript</p>',
    },
  })
}

/** The shell reads storage on mount, so the first paint has to be awaited. */
async function mountSettled() {
  const wrapper = mountShell()
  await flushPromises()
  return wrapper
}

function widthOf(
  wrapper: { get: (selector: string) => { attributes: (name: string) => string | undefined } },
  selector: string,
  attribute: string,
): number {
  return Number(wrapper.get(selector).attributes(attribute))
}

beforeEach(() => {
  localStorage.clear()
})

describe('shell width clamp', () => {
  it('keeps the rail inside the clamp the spec states', () => {
    expect(clampRailWidth(10, 1600)).toBe(240)
    expect(clampRailWidth(288, 1600)).toBe(288)
    expect(clampRailWidth(900, 1600)).toBe(420)
    // The transcript keeps 360 px whatever the rail asks for.
    expect(clampRailWidth(900, 700)).toBe(340)
  })

  it('round-trips the width through storage and tolerates a bad value', () => {
    expect(readShellWidths()).toEqual({ rail: null })

    writeShellWidths({ rail: 312 })
    expect(readShellWidths()).toEqual({ rail: 312 })

    localStorage.setItem(RAIL_WIDTH_STORAGE_KEY, 'wide')
    expect(readShellWidths()).toEqual({ rail: null })
  })

  it('opens the shell at the width it stored', async () => {
    writeShellWidths({ rail: 300 })
    const wrapper = await mountSettled()

    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(300)
  })

  it('opens at the default when nothing is stored', async () => {
    const wrapper = await mountSettled()

    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(288)
  })

  it('moves the rail with the keyboard and remembers where it landed', async () => {
    const wrapper = await mountSettled()
    const handle = wrapper.get('[data-rail-resize]')

    await handle.trigger('keydown', { key: 'ArrowRight' })
    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(304)
    expect(localStorage.getItem(RAIL_WIDTH_STORAGE_KEY)).toBe('304')

    await handle.trigger('keydown', { key: 'Home' })
    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(240)
    expect(localStorage.getItem(RAIL_WIDTH_STORAGE_KEY)).toBe('240')

    await handle.trigger('keydown', { key: 'End' })
    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(420)
  })

  it('carries the clamp into the styles so the pointer path cannot leave it', async () => {
    const wrapper = await mountSettled()

    const rail = wrapper.get('[data-rail]').attributes('style') ?? ''
    expect(rail).toContain('--rail-width: 288px')
  })

  it('exposes the handle as a separator a reader can name and operate', async () => {
    const wrapper = await mountSettled()
    const handle = wrapper.get('[data-rail-resize]')

    expect(handle.attributes('role')).toBe('separator')
    expect(handle.attributes('aria-orientation')).toBe('vertical')
    expect(handle.attributes('aria-valuenow')).toBeTruthy()
    expect(handle.attributes('aria-valuemin')).toBeTruthy()
    expect(handle.attributes('aria-valuemax')).toBeTruthy()
    expect((handle.attributes('aria-label') ?? '').length).toBeGreaterThan(0)
    expect(handle.attributes('tabindex')).toBe('0')
  })
})