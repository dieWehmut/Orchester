import {
  AGENT_FLEET_FIXTURE,
  type BootstrapDto,
  type SessionDetailDto,
  type SessionSummaryDto,
  eventId,
  runId,
  UI_SCHEMA_VERSION,
  type UiEventEnvelope,
} from '@orchester/protokoll'
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import type { HttpClient } from '../src/api/http'
import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

const summary: SessionSummaryDto = {
  id: 's-11111111111111111111111111111111',
  source: 'delegate',
  recorded_at_unix: 1_700_000_000,
  title: 'Inspect the runtime',
  agent: 'codex',
  model: 'gpt-5',
  outcome: 'success',
  resumable: true,
}

const detail: SessionDetailDto = {
  ...summary,
  schema_version: 1,
  prompt: 'Inspect the runtime boundaries.',
  final_text: 'The runtime boundary is isolated.',
  usage: {
    input_tokens: 20,
    output_tokens: 10,
    cached_input_tokens: 5,
    reasoning_output_tokens: 2,
  },
}

describe('WorkspaceView', () => {
  it('hides the centered mark as soon as the active run starts', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-orchester-mark]')).toBeTruthy()
    stores.run.conversationStarted.value = true
    await nextTick()

    expect(wrapper.find('[data-orchester-mark]').exists()).toBe(false)
    expect(wrapper.get('[data-run-awaiting-events]')).toBeTruthy()
  })

  it('places the agent fleet below sessions in the shared left rail', () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-pane="sessions"] [data-agent-fleet]')).toBeTruthy()
    expect(wrapper.get('[data-pane="sessions"] [data-agent-id="codex-main"]')).toBeTruthy()
  })

  it('passes bootstrap workspace and model catalog state to the empty composer', () => {
    const stores = createAppStores()
    stores.bootstrap.context.value = {
      schema_version: 1,
      service_version: '0.1.2',
      server_state: 'running',
      workspace: { selected: true, name: 'Orchester' },
    } satisfies BootstrapDto
    stores.bootstrap.status.value = 'ready'
    stores.models.catalog = MODEL_CATALOG_FIXTURE
    stores.models.status = 'ready'

    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-project-context]').text()).toContain('Orchester')
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
  })

  it('connects the session rail, selected transcript, and inspector to application stores', async () => {
    const http = {
      get: vi.fn(async (path: string) => {
        if (path.startsWith('/sessions/')) return detail
        return { schema_version: 1, items: [summary], next_cursor: null }
      }),
    } as unknown as HttpClient
    const stores = createAppStores({ http })
    stores.bootstrap.context.value = {
      schema_version: 1,
      service_version: '0.1.2',
      server_state: 'running',
      workspace: { selected: true, name: 'Orchester' },
    } satisfies BootstrapDto
    stores.bootstrap.status.value = 'ready'
    stores.sessions.items.value = [summary]
    stores.sessions.status.value = 'ready'

    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    await wrapper.get('[data-session-id]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-pane="sessions"]').text()).toContain(summary.title)
    expect(wrapper.get('[data-session-transcript]').text()).toContain(detail.final_text)
    expect(wrapper.find('[data-pane="inspector"]').exists()).toBe(true)
  })

  it('projects active run file changes into the inspector changes tab', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    const first = fileChangeEvent(1, 'src/app.ts', 'add')
    const latest = fileChangeEvent(2, 'src/app.ts', 'update')

    stores.run.applyEvent(first)
    stores.run.applyEvent(latest)
    await nextTick()
    await wrapper.findAll('[role="tab"]')[2]?.trigger('click')

    expect(wrapper.get('[data-change-path="src/app.ts"]').text()).toContain('Modified')
    expect(wrapper.get('[data-change-path="src/app.ts"]').text()).toContain('2 events')
  })
})

function fileChangeEvent(
  sequence: number,
  path: string,
  kind: 'add' | 'update' | 'delete',
): UiEventEnvelope {
  return {
    schema_version: UI_SCHEMA_VERSION,
    event_id: eventId(`event-${sequence}`),
    run_id: runId('run-changes'),
    sequence,
    occurred_at: `2026-08-21T00:00:0${sequence}Z`,
    kind: { type: 'file_change', path, kind },
  }
}
