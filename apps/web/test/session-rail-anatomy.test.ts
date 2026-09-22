import type { SessionSummaryDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import SessionRail from '../src/components/sessions/SessionRail.vue'

/**
 * The list's anatomy inside the rail.
 *
 * The reference heads a list once, in the column, and keeps one new-chat
 * control at the top of it. A list that prints its own uppercase title under
 * the heading that already names it says the same word twice, and a second
 * "new session" button inside the list is the same action offered twice in one
 * column - so both belong to the rail rather than to the list.
 */

const session = (overrides: Partial<SessionSummaryDto> = {}): SessionSummaryDto => ({
  id: 's-11111111111111111111111111111111',
  source: 'delegate',
  recorded_at_unix: 1_700_000_000,
  title: 'Inspect the runtime',
  agent: 'codex',
  model: 'gpt-5',
  outcome: 'success',
  resumable: true,
  ...overrides,
})

function mountRail(items: SessionSummaryDto[]) {
  return mount(SessionRail, {
    props: { status: 'ready', items, selectedId: null, nextCursor: null, error: null },
  })
}

describe('SessionRail anatomy', () => {
  it('leaves the naming to the rail heading, which already names it', () => {
    const wrapper = mountRail([session()])

    expect(wrapper.find('h2').exists()).toBe(false)
  })

  it('still names the list for assistive technology after losing the title', () => {
    const wrapper = mountRail([session()])

    // The visible heading is the rail's; the list keeps its own accessible
    // name, because a screen reader reaches the list, not the column.
    expect(wrapper.get('nav').attributes('aria-label')).toBe('Sessions')
  })

  it('leaves the new chat control to the rail, so the column has one', () => {
    const wrapper = mountRail([session()])

    expect(wrapper.text()).not.toContain('New session')
    expect(wrapper.find('[data-session-new]').exists()).toBe(false)
  })
})