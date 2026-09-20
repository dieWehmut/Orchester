import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import AppShell from '../src/components/layout/AppShell.vue'
import {
  INSPECTOR_WIDTH_STORAGE_KEY,
  RAIL_WIDTH_STORAGE_KEY,
  clampInspectorWidth,
  clampRailWidth,
  readShellWidths,
  writeShellWidths,
} from '../src/components/layout/shell-widths'

/**
 * The shell's two resizable clamps, section 2.1 of the design spec.
 *
 * The rail and the inspector are the two surfaces a user tunes once and then
 * expects to stay tuned, so each one carries a clamp rather than a fixed width
 * and each remembers what it was set to. The clamps are the rule the spec
 * states: the rail must leave the transcript its 360 px, and the inspector sits
 * between its own minimum and maximum regardless of how far the pointer travels.
 */

function mountShell() {
  return mount(AppShell, {
    props: { sessionsTitle: 'Sessions', inspectorTitle: 'Inspector' },
    slots: {
      sessions: '<p>Sessions</p>',
      default: '<p>Transcript</p>',
      inspector: '<p>Inspector</p>',
    },
  })
}

/** The shell reads storage on mount, so the first paint has to be awaited. */
async function mountSettled() {
  const wrapper = mountShell()
  await flushPromises()
  return wrapper
}

function widthOf(wrapper: { get: (selector: string) => { attributes: (name: string) => string | undefined } }, selector: string, attribute: string) {
  return Number(wrapper.get(selector).attributes(attribute))
}

beforeEach(() => {
  localStorage.clear()
})

describe('shell width clamps', () => {
  it('keeps the rail inside the clamp the spec states', () => {
    expect(clampRailWidth(10, 1600)).toBe(240)
    expect(clampRailWidth(288, 1600)).toBe(288)
    expect(clampRailWidth(900, 1600)).toBe(420)
    // The transcript keeps 360 px whatever the rail asks for.
    expect(clampRailWidth(900, 700)).toBe(340)
  })

  it('keeps the inspector inside its own clamp', () => {
    expect(clampInspectorWidth(10)).toBe(280)
    expect(clampInspectorWidth(340)).toBe(340)
    expect(clampInspectorWidth(900)).toBe(460)
  })

  it('round-trips the widths through storage and tolerates a bad value', () => {
    expect(readShellWidths()).toEqual({ rail: null, inspector: null })

    writeShellWidths({ rail: 312, inspector: 360 })
    expect(readShellWidths()).toEqual({ rail: 312, inspector: 360 })

    localStorage.setItem(RAIL_WIDTH_STORAGE_KEY, 'wide')
    localStorage.setItem(INSPECTOR_WIDTH_STORAGE_KEY, '-4')
    expect(readShellWidths()).toEqual({ rail: null, inspector: null })
  })

  it('opens the shell at the widths it stored', async () => {
    writeShellWidths({ rail: 300, inspector: 380 })
    const wrapper = await mountSettled()

    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(300)
    expect(widthOf(wrapper, '[data-inspector]', 'data-inspector-width')).toBe(380)
  })

  it('opens at the defaults when nothing is stored', async () => {
    const wrapper = await mountSettled()

    expect(widthOf(wrapper, '[data-rail]', 'data-rail-width')).toBe(288)
    expect(widthOf(wrapper, '[data-inspector]', 'data-inspector-width')).toBe(340)
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

  it('moves the inspector boundary the way the handle points', async () => {
    const wrapper = await mountSettled()
    const handle = wrapper.get('[data-inspector-resize]')

    // The handle is the inspector's left boundary, so dragging it left gives
    // the inspector more room and dragging it right gives it less.
    await handle.trigger('keydown', { key: 'ArrowLeft' })
    expect(widthOf(wrapper, '[data-inspector]', 'data-inspector-width')).toBe(356)
    expect(localStorage.getItem(INSPECTOR_WIDTH_STORAGE_KEY)).toBe('356')

    await handle.trigger('keydown', { key: 'ArrowRight' })
    expect(widthOf(wrapper, '[data-inspector]', 'data-inspector-width')).toBe(340)

    await handle.trigger('keydown', { key: 'End' })
    expect(widthOf(wrapper, '[data-inspector]', 'data-inspector-width')).toBe(460)
  })

  it('carries the clamps into the styles so the pointer path cannot leave them', async () => {
    const wrapper = await mountSettled()

    const rail = wrapper.get('[data-rail]').attributes('style') ?? ''
    expect(rail).toContain('--rail-width: 288px')

    const inspector = wrapper.get('[data-inspector]').attributes('style') ?? ''
    expect(inspector).toContain('--inspector-width: 340px')
  })

  it('exposes the handles as separators a reader can name and operate', async () => {
    const wrapper = await mountSettled()

    for (const selector of ['[data-rail-resize]', '[data-inspector-resize]']) {
      const handle = wrapper.get(selector)
      expect(handle.attributes('role'), selector).toBe('separator')
      expect(handle.attributes('aria-orientation'), selector).toBe('vertical')
      expect(handle.attributes('aria-valuenow'), selector).toBeTruthy()
      expect(handle.attributes('aria-valuemin'), selector).toBeTruthy()
      expect(handle.attributes('aria-valuemax'), selector).toBeTruthy()
      expect((handle.attributes('aria-label') ?? '').length, selector).toBeGreaterThan(0)
      expect(handle.attributes('tabindex'), selector).toBe('0')
    }
  })
})
