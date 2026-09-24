import type { SessionSummaryDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import TitleRow from '../src/components/layout/TitleRow.vue'
import SessionRail from '../src/components/sessions/SessionRail.vue'

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

describe('workspace components', () => {
  it('says nothing about the connection while the runtime is ready', () => {
    // The product and the workspace are named by the rail's product row since
    // U26, so the row's remaining readout is trouble: a ready runtime is the
    // state the reader assumes, and a permanent "Connected" is noise the
    // reference's own top strip does not carry.
    const wrapper = mount(TitleRow, { props: { connection: 'ready' } })

    expect(wrapper.find('[data-testid="connection-label"]').exists()).toBe(false)
  })

  it('reports the connection while it is not ready', () => {
    const wrapper = mount(TitleRow, { props: { connection: 'pending' } })

    expect(wrapper.get('[data-testid="connection-label"]').text()).toBe('Runtime pending')
  })

  it('renders session state and emits selection from a real button', async () => {
    const wrapper = mount(SessionRail, {
      props: {
        status: 'ready',
        items: [session],
        selectedId: null,
        nextCursor: null,
        error: null,
      },
    })

    await wrapper.get('[data-session-id]').trigger('click')

    expect(wrapper.emitted('select')).toEqual([[session.id]])
    expect(wrapper.get('[data-session-id]').attributes('aria-pressed')).toBe('false')
  })
})
