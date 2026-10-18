import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { createEmptyRunView } from '@orchester/ereignis'
import RunTimeline from '../src/components/run/RunTimeline.vue'

it('retains scroll space for offscreen rows and remeasures growing or expanded content', async () => {
  let resized = () => {}
  const disconnect = vi.fn()
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { resized = callback }
    observe() {}
    disconnect = disconnect
  })
  let firstHeight = 100
  const boxes = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    return { height: this.dataset.virtualizedTurn === '0' ? firstHeight : 100 } as DOMRect
  })
  const view = { ...createEmptyRunView(), timeline: Array.from({ length: 30 }, (_, i) => ({
    type: 'message' as const, key: `reply-${i}`, sequence: i + 1, occurredAt: '2026-10-18T00:00:00Z', turnId: null,
    role: 'assistant' as const, text: `Reply ${i}`, final: true,
  })) }
  const wrapper = mount(RunTimeline, { props: { view, viewportHeight: 200 } })
  try {
    await flushPromises()
    expect(wrapper.get('[data-virtual-spacer="bottom"]').attributes('style')).toContain('2600px')
    firstHeight = 400; resized(); await flushPromises()
    expect(wrapper.get('[data-virtual-spacer="bottom"]').attributes('style')).toContain('2700px')
    expect(wrapper.emitted('resize')!.length).toBeGreaterThan(1)
    await wrapper.setProps({ scrollTop: 2800 }); await flushPromises()
    expect(wrapper.get('[data-virtual-spacer="top"]').attributes('style')).toContain('2600px')
    expect(wrapper.get('[data-virtual-spacer="bottom"]').attributes('style')).toContain('100px')
    await wrapper.setProps({ scrollTop: 3100 }); await flushPromises()
    expect(wrapper.get('[data-virtual-spacer="top"]').attributes('style')).toContain('2900px')
    expect(wrapper.text()).toContain('Reply 29')
  } finally { wrapper.unmount(); boxes.mockRestore(); vi.unstubAllGlobals() }
  expect(disconnect).toHaveBeenCalled()
})
