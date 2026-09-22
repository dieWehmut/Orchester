import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import MessageActions from '../src/components/run/MessageActions.vue'

/**
 * What a reader can do with a message, and what each control admits to doing.
 *
 * The row is the surface's own set of answers: copying is always there, and the
 * actions a surface adds arrive with their own words and their own glyph, so a
 * control cannot be added here without saying what it does. The reference hangs
 * a row of icons off a message; this is the same shape with nothing in it that
 * does not do something.
 */

const writes: string[] = []

beforeEach(() => {
  writes.length = 0
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: {
      writeText: (value: string) => {
        writes.push(value)
        return Promise.resolve()
      },
    },
  })
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('MessageActions', () => {
  it("draws copy first and the surface's own actions after it", () => {
    const wrapper = mount(MessageActions, {
      props: {
        text: 'the answer',
        label: 'Copy message',
        copiedLabel: 'Copied',
        actions: [{ id: 'quote', label: 'Quote in the composer', icon: 'quote' }],
      },
    })

    const order = wrapper.findAll('button').map((button) =>
      button.attributes('data-message-copy') !== undefined
        ? 'copy'
        : button.attributes('data-message-action'),
    )

    expect(order).toEqual(['copy', 'quote'])
    expect(wrapper.get('[data-message-action="quote"]').attributes('aria-label')).toBe(
      'Quote in the composer',
    )
  })

  it('reports which action was asked for, without copying anything', async () => {
    const wrapper = mount(MessageActions, {
      props: {
        text: 'the answer',
        label: 'Copy message',
        copiedLabel: 'Copied',
        actions: [{ id: 'reuse', label: 'Use this prompt again', icon: 'reuse' }],
      },
    })

    await wrapper.get('[data-message-action="reuse"]').trigger('click')

    expect(wrapper.emitted('action')).toEqual([['reuse']])
    expect(writes).toEqual([])
  })

  it('draws no extra action when the surface has none to offer', () => {
    const wrapper = mount(MessageActions, {
      props: { text: 'the answer', label: 'Copy message', copiedLabel: 'Copied' },
    })

    expect(wrapper.findAll('button')).toHaveLength(1)
  })
})