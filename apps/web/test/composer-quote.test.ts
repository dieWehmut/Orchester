import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'

import RunPanel from '../src/components/run/RunPanel.vue'

/**
 * The gesture the action row exists for.
 *
 * The reference offers more than copying, and the one thing this product can
 * honestly add is the reader's next move: put the answer into the composer as a
 * quotation, or put their own prompt back to run again. Neither is a button that
 * starts work behind the reader's back - both end with the caret in the field,
 * which is what makes the action a step rather than a decision.
 */

function viewWithAssistant(text: string) {
  return {
    ...createEmptyRunView(),
    timeline: [
      {
        type: 'message' as const,
        key: 'message-user',
        sequence: 1,
        occurredAt: '2026-09-20T06:00:00Z',
        turnId: null,
        role: 'user' as const,
        text: 'Inspect the runtime.',
        final: true,
      },
      {
        type: 'message' as const,
        key: 'message-assistant',
        sequence: 2,
        occurredAt: '2026-09-20T06:00:01Z',
        turnId: null,
        role: 'assistant' as const,
        text,
        final: true,
      },
    ],
  }
}

function mounted(view: ReturnType<typeof viewWithAssistant>) {
  return mount(RunPanel, { props: { view }, attachTo: document.body })
}

describe('composer gestures', () => {
  it('quotes the answer into the composer, line by line, and takes the caret', async () => {
    const wrapper = mounted(viewWithAssistant('First line.\nSecond line.'))

    await wrapper.get('[data-message-action="quote"]').trigger('click')
    await wrapper.vm.$nextTick()

    const field = wrapper.get('textarea').element as HTMLTextAreaElement

    // The quotation is markdown, because that is the language the field is read
    // in: a quoted block is what the agent will see when this is sent.
    expect(field.value).toBe('> First line.\n> Second line.\n\n')
    expect(document.activeElement).toBe(field)
    wrapper.unmount()
  })

  it("puts the reader's own prompt back, so it can be run again", async () => {
    const wrapper = mounted(viewWithAssistant('An answer.'))

    await wrapper.get('[data-message-action="reuse"]').trigger('click')
    await wrapper.vm.$nextTick()

    const field = wrapper.get('textarea').element as HTMLTextAreaElement

    expect(field.value).toBe('Inspect the runtime.')
    expect(document.activeElement).toBe(field)
    wrapper.unmount()
  })
})