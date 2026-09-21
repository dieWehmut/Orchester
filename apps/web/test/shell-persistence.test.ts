import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import WorkspaceView from '../src/views/WorkspaceView.vue'
import { createAppStores } from '../src/stores/app'
import {
  TAB_STRIP_STATE_STORAGE_KEY,
  readTabStripState,
  writeTabStripState,
} from '../src/components/layout/tab-strip-persistence'
import { RAIL_WIDTH_STORAGE_KEY, writeShellWidths } from '../src/components/layout/shell-widths'

/**
 * Remembering the shell across a restart, section 2 of the design spec.
 *
 * The spec's closing line is that the widths are persisted per user, and
 * section 4.2 adds the tabs to the same promise: the runtime must persist open
 * tabs. The two live in one place - the shell's own storage - rather than in
 * the routes or the stores, because a tab and a width are both facts about the
 * shell rather than about the work inside it.
 *
 * The panel widths were already remembered by U2-02. What this pins is that
 * they are remembered through the same path as the tabs, and that a tab whose
 * surface no longer exists does not come back.
 */

function mountWorkspace() {
  const stores = createAppStores()
  return mount(WorkspaceView, { global: { plugins: [stores] } })
}

beforeEach(() => {
  localStorage.clear()
})

describe('shell persistence', () => {
  it('round-trips the tab strip through storage and rejects what is not a tab list', () => {
    expect(readTabStripState()).toEqual({ order: [], activeId: null })

    writeTabStripState({ order: ['run', 'inspector'], activeId: 'inspector' })
    expect(readTabStripState()).toEqual({ order: ['run', 'inspector'], activeId: 'inspector' })

    localStorage.setItem(TAB_STRIP_STATE_STORAGE_KEY, '{"order":["run",3]}')
    expect(readTabStripState()).toEqual({ order: [], activeId: null })

    localStorage.setItem(TAB_STRIP_STATE_STORAGE_KEY, '["run"]')
    expect(readTabStripState()).toEqual({ order: [], activeId: null })
  })

  it('restores the tab order and the selection the user left', async () => {
    writeTabStripState({ order: ['run', 'inspector'], activeId: 'inspector' })
    const wrapper = mountWorkspace()
    await flushPromises()

    const strip = wrapper.get('[data-testid="workspace-tab-strip"]')
    expect(strip.attributes('data-tabstrip-overflow')).toBe('false')
    expect(wrapper.get('[data-tabstrip-tab="inspector"]').attributes('aria-selected')).toBe('true')
  })

  it('writes the order back when the user reorders a tab', async () => {
    const wrapper = mountWorkspace()
    await flushPromises()

    const from = wrapper.get('[data-tabstrip-tab="inspector"]')
    const onto = wrapper.get('[data-tabstrip-tab="run"]')
    await from.trigger('dragstart')
    await onto.trigger('drop')
    await flushPromises()

    expect(readTabStripState().order).toEqual(['inspector', 'run'])
  })

  it('writes the selection back when the user picks another tab', async () => {
    const wrapper = mountWorkspace()
    await flushPromises()

    await wrapper.get('[data-tabstrip-tab="inspector"]').trigger('click')
    await flushPromises()

    expect(readTabStripState().activeId).toBe('inspector')
    expect(wrapper.get('[data-tabstrip-tab="inspector"]').attributes('aria-selected')).toBe('true')
  })

  it('keeps the widths and the tabs in one place, so a restart restores both', async () => {
    writeShellWidths({ rail: 320, inspector: 400 })
    writeTabStripState({ order: ['run'], activeId: 'run' })

    const wrapper = mountWorkspace()
    await flushPromises()

    expect(localStorage.getItem(RAIL_WIDTH_STORAGE_KEY)).toBe('320')
    expect(wrapper.get('[data-rail]').attributes('data-rail-width')).toBe('320')
    expect(wrapper.get('[data-inspector]').attributes('data-inspector-width')).toBe('400')
  })
})
