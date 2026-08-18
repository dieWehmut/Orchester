import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'

import ToastRegion from '../src/components/ToastRegion.vue'

const items = [
  { id: 'saved', message: 'Run saved', tone: 'success' as const },
  { id: 'failed', title: 'Provider failed', message: 'Try again.', tone: 'error' as const },
]

afterEach(() => {
  vi.useRealTimers()
  document.body.replaceChildren()
})

describe('ToastRegion', () => {
  it('renders a bounded queue and emits the dismissed item id', async () => {
    const wrapper = mount(ToastRegion, {
      attachTo: document.body,
      props: { items, maxVisible: 2, timeout: 0 },
    })

    expect(wrapper.attributes('aria-label')).toBe('Notifications')
    expect(wrapper.findAll('[data-toast-item]')).toHaveLength(2)

    await wrapper.find('[data-toast-dismiss]').trigger('click')
    expect(wrapper.emitted('dismiss')).toEqual([['saved']])
  })

  it('emits dismiss after an item timeout', async () => {
    vi.useFakeTimers()
    const wrapper = mount(ToastRegion, {
      props: { items: [items[0]!], timeout: 500 },
    })

    await vi.advanceTimersByTimeAsync(499)
    expect(wrapper.emitted('dismiss')).toBeUndefined()
    await vi.advanceTimersByTimeAsync(1)
    expect(wrapper.emitted('dismiss')).toEqual([['saved']])
  })

  it('keeps only the newest items when the queue exceeds maxVisible', () => {
    const wrapper = mount(ToastRegion, {
      props: {
        items: [
          ...items,
          { id: 'third', message: 'Third', tone: 'info' as const },
        ],
        maxVisible: 2,
        timeout: 0,
      },
    })

    expect(wrapper.findAll('[data-toast-item]')).toHaveLength(2)
    expect(wrapper.text()).not.toContain('Run saved')
    expect(wrapper.text()).toContain('Third')
  })
})
