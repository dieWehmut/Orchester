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

  it('files the runs under the project they ran in, as the reference does', () => {
    const wrapper = mount(SessionRail, {
      props: {
        status: 'ready',
        items: [
          { ...session, id: 's-nexus-1', project: 'Nexus' },
          { ...session, id: 's-orchester-1', project: 'Orchester' },
          { ...session, id: 's-nexus-2', project: 'Nexus' },
          { ...session, id: 's-unfiled-1', project: null },
        ],
        selectedId: null,
        nextCursor: null,
        error: null,
      },
    })

    const groups = wrapper.findAll('[data-session-project]')
    expect(groups.map((group) => group.attributes('data-session-project'))).toEqual([
      'Nexus',
      'Orchester',
      '',
    ])
    // The project's own row names it, and its runs sit inside that group.
    expect(groups[0]!.get('[data-session-project-name]').text()).toContain('Nexus')
    expect(groups[0]!.findAll('[data-session-id]')).toHaveLength(2)
    // The runs that named no project are drawn without a name of their own: the
    // rail does not invent a project a session never ran in.
    expect(groups[2]!.find('[data-session-project-name]').exists()).toBe(false)
    expect(groups[2]!.findAll('[data-session-id]')).toHaveLength(1)
  })
})
