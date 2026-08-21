import type { SessionSummaryDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { h } from 'vue'

import InspectorDock from '../src/components/layout/InspectorDock.vue'
import WorkspaceHeader from '../src/components/layout/WorkspaceHeader.vue'
import SessionRail from '../src/components/sessions/SessionRail.vue'
import ChangeInspector from '../src/components/changes/ChangeInspector.vue'
import type { ChangeSummary } from '../src/components/changes/change-summary'

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
  it('renders runtime identity in the workspace header', () => {
    const wrapper = mount(WorkspaceHeader, {
      props: { connection: 'ready', workspaceName: 'Orchester' },
    })

    expect(wrapper.get('[data-testid="product-name"]').text()).toBe('Orchester')
    expect(wrapper.get('[data-testid="workspace-name"]').text()).toBe('Orchester')
    expect(wrapper.get('[data-testid="connection-label"]').text()).toBe('Connected')
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

  it('keeps inspector sections reachable as tabs', async () => {
    const wrapper = mount(InspectorDock)

    await wrapper.findAll('[role="tab"]')[1]?.trigger('click')

    expect(wrapper.get('[data-inspector-panel]').text()).toContain('Approvals')
  })

  it('renders a named changes slot when the changes tab is selected', async () => {
    const changes: ChangeSummary[] = [
      {
        path: 'src/app.ts',
        kind: 'update',
        latestSequence: 3,
        latestOccurredAt: '2026-08-21T00:00:03Z',
        eventCount: 2,
        history: [],
      },
    ]
    const wrapper = mount(InspectorDock, {
      slots: { changes: () => h(ChangeInspector, { changes }) },
    })

    await wrapper.findAll('[role="tab"]')[2]?.trigger('click')

    expect(wrapper.get('[data-change-path="src/app.ts"]').text()).toContain('src/app.ts')
  })

  it('accepts a controlled active inspector tab', () => {
    const wrapper = mount(InspectorDock, { props: { activeTab: 'changes' } })

    expect(wrapper.findAll('[role="tab"]')[2]?.attributes('aria-selected')).toBe('true')
    expect(wrapper.get('[data-inspector-panel]').text()).toContain('Changes')
  })
})
