import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { resetAppearanceForTests, useAppearance } from '@orchester/design'
import { createEmptyRunView } from '@orchester/ereignis'

import RunPanel from '../src/components/run/RunPanel.vue'
import SettingsView from '../src/views/SettingsView.vue'
import { resetPetPackForTests, resetPetVisibilityForTests } from '../src/features/pet'

beforeEach(() => {
  localStorage.clear()
  resetAppearanceForTests()
  resetPetPackForTests()
  resetPetVisibilityForTests()
  vi.stubGlobal('fetch', async () => new Response(JSON.stringify({
    id: 'xiaoxuan', displayName: '小萱', description: 'Companion',
    spriteVersionNumber: 2, spritesheetPath: 'spritesheet.webp',
  })))
})

afterEach(() => {
  vi.unstubAllGlobals()
  resetAppearanceForTests()
  localStorage.clear()
})

describe('integrated companion preferences', () => {
  it('finds the companion visibility setting through settings search', async () => {
    const wrapper = mount(SettingsView)
    try {
      await wrapper.get('[data-settings-search] input').setValue('companion')
      expect(wrapper.findAll('[data-settings-nav-link]').map((item) =>
        item.attributes('data-settings-nav-link'))).toEqual(['pet'])
      await wrapper.get('[data-settings-nav-link="pet"]').trigger('click')
      expect(wrapper.get('[data-settings-section="pet"]').attributes('hidden')).toBeUndefined()
    } finally {
      wrapper.unmount()
    }
  })

  it('keeps the companion still when the shared motion preference is reduced', async () => {
    const appearance = useAppearance()
    appearance.setReducedMotion('true')
    const wrapper = mount(RunPanel, {
      // A page with a run on it: the companion is drawn where a run is, not on
      // the home page before the first one.
      props: { view: createEmptyRunView(), conversationStarted: true, runStatus: 'running' },
      attachTo: document.body,
    })
    try {
      await flushPromises()
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 500, clientY: 0 }))
      await flushPromises()
      const companion = wrapper.get('[data-pet-companion]')
      expect(companion.attributes('data-pet-look')).toBeUndefined()
      expect(companion.attributes('data-pet-frame')).toBe('56')
      appearance.setReducedMotion('false')
      await flushPromises()
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 500, clientY: 0 }))
      await flushPromises()
      expect(companion.attributes('data-pet-look')).toBeDefined()
    } finally {
      wrapper.unmount()
    }
  })
})
