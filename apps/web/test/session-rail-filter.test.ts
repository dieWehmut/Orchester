import type { SessionSummaryDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import SessionRail from '../src/components/sessions/SessionRail.vue'

/**
 * The filter the rail's header search drives.
 *
 * The reference sidebar puts a search action in its header, above the lists
 * rather than inside one of them, so it reads as "narrow what this column
 * shows" rather than "search one list". The runtime's `/sessions` route takes
 * a cursor and a limit and nothing else, so the honest implementation filters
 * the page the rail already holds - and says so when nothing matches instead
 * of rendering an empty list that looks like a load failure.
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

function mountRail(items: SessionSummaryDto[], query?: string) {
  return mount(SessionRail, {
    props: {
      status: 'ready',
      items,
      selectedId: null,
      nextCursor: null,
      error: null,
      ...(query === undefined ? {} : { query }),
    },
  })
}

describe('SessionRail filter', () => {
  it('narrows the list to the sessions whose text matches', () => {
    const wrapper = mountRail(
      [
        session({ id: 's-aaaa', title: 'Inspect the runtime' }),
        session({ id: 's-bbbb', title: 'Refactor the composer' }),
      ],
      'composer',
    )

    const rows = wrapper.findAll('[data-session-id]')
    expect(rows).toHaveLength(1)
    expect(rows[0]!.attributes('data-session-id')).toBe('s-bbbb')
  })

  it('matches the agent and model as well as the title', () => {
    const wrapper = mountRail(
      [
        session({ id: 's-aaaa', title: 'One', agent: 'codex', model: 'gpt-5' }),
        session({ id: 's-bbbb', title: 'Two', agent: 'claude', model: 'sonnet' }),
      ],
      'sonnet',
    )

    expect(wrapper.findAll('[data-session-id]')).toHaveLength(1)
    expect(wrapper.get('[data-session-id]').attributes('data-session-id')).toBe('s-bbbb')
  })

  it('says the filter matched nothing rather than showing an empty list', () => {
    const wrapper = mountRail([session({ title: 'Inspect the runtime' })], 'nothing here')

    expect(wrapper.findAll('[data-session-id]')).toHaveLength(0)
    expect(wrapper.get('[data-session-filter-empty]').text()).toContain('No sessions match')
  })

  it('keeps every session when the filter is empty', () => {
    const wrapper = mountRail([session({ id: 's-aaaa' }), session({ id: 's-bbbb' })], '   ')

    expect(wrapper.findAll('[data-session-id]')).toHaveLength(2)
    expect(wrapper.find('[data-session-filter-empty]').exists()).toBe(false)
  })
})
