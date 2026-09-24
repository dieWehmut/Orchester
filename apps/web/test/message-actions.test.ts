import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import MessageActions from '../src/components/run/MessageActions.vue'

/**
 * What a reader can do with a message, and what each control admits to doing.
 *
 * The reference's row is a few controls in the open and a `…` for the rest, and
 * this is the same shape: copy is always there, the move that belongs to an
 * answer - running its question again - is drawn when the surface has a question
 * to run, and whatever else a surface can answer lives in the menu. Nothing is
 * drawn here that does not do something.
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

function mountRow(props: Record<string, unknown> = {}) {
  return mount(MessageActions, {
    props: {
      text: 'the answer',
      label: 'Copy message',
      copiedLabel: 'Copied',
      moreLabel: 'More actions',
      ...props,
    },
  })
}

describe('MessageActions', () => {
  it('draws copy first, the answer’s own move second and the rest in the menu', () => {
    const wrapper = mountRow({
      rerunLabel: 'Run this question again',
      actions: [{ id: 'quote', label: 'Quote in the composer' }],
    })

    const controls = wrapper.findAll('button').map((button) => {
      if (button.attributes('data-message-copy') !== undefined) return 'copy'
      if (button.attributes('data-message-rerun') !== undefined) return 'rerun'
      if (button.classes().includes('app-menu__trigger')) return 'more'
      return 'menu-item'
    })

    expect(controls).toEqual(['copy', 'rerun', 'more'])
    expect(wrapper.get('[data-message-rerun]').attributes('aria-label')).toBe(
      'Run this question again',
    )
    // The menu's own trigger carries the name the row gave it.
    expect(wrapper.get('.app-menu__trigger').attributes('aria-label')).toBe('More actions')
  })

  it('copies the message and says so, and nothing else', async () => {
    const wrapper = mountRow()

    await wrapper.get('[data-message-copy]').trigger('click')

    expect(writes).toEqual(['the answer'])
    expect(wrapper.get('[data-message-copy-status]').text()).toBe('Copied')
    expect(wrapper.emitted('action')).toBeUndefined()
  })

  it('reports the answer’s own move without copying anything', async () => {
    const wrapper = mountRow({ rerunLabel: 'Run this question again' })

    await wrapper.get('[data-message-rerun]').trigger('click')

    expect(wrapper.emitted('action')).toEqual([['rerun']])
    expect(writes).toEqual([])
  })

  it('draws no such move when the surface has no question to run', () => {
    const wrapper = mountRow()

    // A question cannot be asked again, and a first answer whose question is not
    // in the journal has nothing to run: the control is absent rather than dead.
    expect(wrapper.find('[data-message-rerun]').exists()).toBe(false)
  })

  it('reports a menu action by its own name', async () => {
    const wrapper = mountRow({ actions: [{ id: 'reuse', label: 'Use this prompt again' }] })

    await wrapper.get('.app-menu__trigger').trigger('click')
    await nextTick()

    const item = wrapper
      .findAll('[role="menuitem"]')
      .find((node) => node.text() === 'Use this prompt again')
    expect(item).toBeDefined()
    await item!.trigger('click')

    expect(wrapper.emitted('action')).toEqual([['reuse']])
    expect(writes).toEqual([])
  })

  it('draws no menu when the surface has nothing else to offer', () => {
    const wrapper = mountRow()

    expect(wrapper.find('.app-menu__trigger').exists()).toBe(false)
    expect(wrapper.findAll('button')).toHaveLength(1)
  })
})