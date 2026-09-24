import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'

import RunPanel from '../src/components/run/RunPanel.vue'

/**
 * The gesture the action row exists for.
 *
 * The reference's row keeps a control or two in the open and the rest in a `…`;
 * what this product can honestly add to copying is the reader's next move: put
 * the answer into the composer as a quotation, or put their own prompt back to
 * run again. Neither is a button that starts work behind the reader's back -
 * both end with the caret in the field, which is what makes the action a step
 * rather than a decision.
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

/**
 * Open the menu of the row that holds a given control, and press an action.
 *
 * Chosen by what the row *is* rather than by its position: the answer's row is
 * the one offering to run the question again, the reader's own row is the one
 * that is not, and the transcript's order is not this test's business.
 */
async function chooseFromMenu(
  wrapper: ReturnType<typeof mounted>,
  whose: 'answer' | 'question',
  label: string,
): Promise<void> {
  const rows = wrapper.findAll('[data-message-actions]')
  const row = rows.find((candidate) =>
    whose === 'answer'
      ? candidate.find('[data-message-rerun]').exists()
      : !candidate.find('[data-message-rerun]').exists(),
  )
  if (row === undefined) throw new Error(`no ${whose} row`)

  await row.get('.app-menu__trigger').trigger('click')
  const item = wrapper
    .findAll('[role="menuitem"]')
    .find((node) => node.text() === label)
  if (item === undefined) throw new Error(`no menu item ${label}`)
  await item.trigger('click')
  await wrapper.vm.$nextTick()
}

describe('composer gestures', () => {
  it('quotes the answer into the composer, line by line, and takes the caret', async () => {
    const wrapper = mounted(viewWithAssistant('First line.\nSecond line.'))

    await chooseFromMenu(wrapper, 'answer', 'Quote in the composer')

    const field = wrapper.get('textarea').element as HTMLTextAreaElement

    // The quotation is markdown, because that is the language the field is read
    // in: a quoted block is what the agent will see when this is sent.
    expect(field.value).toBe('> First line.\n> Second line.\n\n')
    expect(document.activeElement).toBe(field)
    wrapper.unmount()
  })

  it("puts the reader's own prompt back, so it can be run again", async () => {
    const wrapper = mounted(viewWithAssistant('An answer.'))

    // The second row is the reader's own turn, which holds the reuse action.
    await chooseFromMenu(wrapper, 'question', 'Use this prompt again')

    const field = wrapper.get('textarea').element as HTMLTextAreaElement

    expect(field.value).toBe('Inspect the runtime.')
    expect(document.activeElement).toBe(field)
    wrapper.unmount()
  })

  it('runs the question an answer answered, from the answer’s own row', async () => {
    const wrapper = mounted(viewWithAssistant('An answer.'))

    await wrapper.get('[data-message-rerun]').trigger('click')

    // The runtime starts a run for the prompt; nothing is put in the field,
    // because the reader asked for another answer rather than the question back.
    expect(wrapper.emitted('submit')).toEqual([['Inspect the runtime.']])
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('')
    wrapper.unmount()
  })
})