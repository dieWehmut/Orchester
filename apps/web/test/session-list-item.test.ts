import type { SessionSummaryDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import SessionListItem from '../src/components/sessions/SessionListItem.vue'

/**
 * One row per run, as the reference draws a list: the title, and the state the
 * reader scans for.
 *
 * The reference's rows are a title and nothing else, and this list had grown a
 * second line of agent, model and resumable. Those are details of a run rather
 * than its name - the pane that opens when the row is chosen states them in
 * full - so they moved behind the row instead of sitting on it: available to a
 * pointer as a tooltip and to a screen reader as hidden text, which is what
 * keeps one line from meaning less. The outcome stays where it was, on the dot,
 * which carries it as its accessible name.
 */

const session: SessionSummaryDto = {
  id: 's-11111111111111111111111111111111',
  source: 'delegate',
  recorded_at_unix: 1_700_000_000,
  title: 'Inspect the runtime',
  agent: 'codex',
  model: 'gpt-5',
  outcome: 'success',
  resumable: true,
}

function mountRow(overrides: Partial<SessionSummaryDto> = {}) {
  return mount(SessionListItem, {
    props: { session: { ...session, ...overrides }, selected: false },
  })
}

describe('SessionListItem', () => {
  it('states the title and when it ran, and nothing else on the row', () => {
    const wrapper = mountRow()
    const button = wrapper.get('[data-session-id]')

    expect(button.get('.session-list-item__title').text()).toBe('Inspect the runtime')
    expect(button.findAll('.session-list-item__time')).toHaveLength(1)

    // The row is the dot, the title, the time and the text a reader hears: the
    // agent, the model and the resumable note are not elements on it, because
    // they are what the row is about rather than what it is called.
    expect(button.element.children).toHaveLength(4)
  })

  it('still says the details to a reader who cannot hover', () => {
    const wrapper = mountRow()
    const details = wrapper.get('[data-session-details]')

    expect(details.text()).toContain('codex')
    expect(details.text()).toContain('gpt-5')
    expect(details.text()).toContain('Resumable')
    // Hidden from the eye, not from the accessibility tree.
    expect(details.classes()).toContain('visually-hidden')
  })

  it('keeps the outcome on the dot, which carries it as its own name', () => {
    const ok = mountRow()
    const failed = mountRow({ outcome: 'failed' })

    expect(ok.get('.status-dot').attributes('aria-label')).toBe('success')
    expect(ok.get('.status-dot').classes()).toContain('status-dot--success')
    expect(failed.get('.status-dot').classes()).toContain('status-dot--error')
    // The dot already says it, so the hidden details do not say it twice.
    expect(ok.get('[data-session-details]').text()).not.toContain('success')
  })

  it('offers the same facts to the pointer as a tooltip', () => {
    const wrapper = mountRow()
    const title = wrapper.get('[data-session-id]').attributes('title') ?? ''

    expect(title).toContain('Inspect the runtime')
    expect(title).toContain('codex')
  })

  it('leaves the model and the resumable note out when the run has neither', () => {
    const wrapper = mountRow({ model: null, resumable: false })
    const details = wrapper.get('[data-session-details]')

    expect(details.text()).toContain('codex')
    expect(details.text()).not.toContain('gpt-5')
    expect(details.text()).not.toContain('Resumable')
  })
})