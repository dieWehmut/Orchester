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

  it('names the composer state from the run store lifecycle', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-run-composer]').attributes('data-composer-state')).toBe('idle')

    stores.run.lifecycle.value = 'submitting'
    await nextTick()
    expect(wrapper.get('[data-run-composer]').attributes('data-composer-state')).toBe('submitting')

    stores.run.lifecycle.value = 'running'
    await nextTick()
    expect(wrapper.get('[data-run-composer]').attributes('data-composer-state')).toBe('running')

    stores.run.lifecycle.value = 'cancelling'
    await nextTick()
    expect(wrapper.get('[data-run-composer]').attributes('data-composer-state')).toBe('cancelling')
  })

  it('places the agent fleet below sessions in the shared left rail', () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    expect(wrapper.get('[data-pane="sessions"] [data-agent-fleet]')).toBeTruthy()
    expect(wrapper.get('[data-pane="sessions"] [data-agent-id="codex-main"]')).toBeTruthy()
  })

  it('keeps the selected agent visible in the shared left rail', async () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    await wrapper.get('[data-agent-id="codex-main"] button').trigger('click')

    expect(wrapper.get('[data-agent-id="codex-main"] button').classes()).toContain(
      'agent-fleet-row--selected',
    )
    expect(wrapper.get('[data-agent-id="codex-main"] button').attributes('aria-pressed')).toBe('true')
  })

  it('opens the agent context inspector after selecting an agent', async () => {
    const stores = createAppStores()
    stores.agents.snapshot = AGENT_FLEET_FIXTURE
    stores.agents.status = 'ready'
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })

    await wrapper.get('[data-agent-id="codex-main"] button').trigger('click')

    expect(wrapper.get('[data-agent-details-name]').text()).toBe('Codex')
    expect(wrapper.findAll('[role="tab"]')[0]?.attributes('aria-selected')).toBe('true')
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

  it('connects the session rail, selected transcript, and the run surfaces to application stores', async () => {
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
    // The run's surfaces are the panel's now: the shell draws no right column,
    // so what has to be true is that the panel carries the tabs.
    expect(
      wrapper
        .findAll('[data-bottom-panel-tab]')
        .map((tab) => tab.attributes('data-bottom-panel-tab')),
    ).toEqual(['context', 'approvals', 'changes', 'terminal', 'output', 'audit'])
  })

  it('opens the review surface from the strip tab that names it', async () => {
    const stores = createAppStores()
    const wrapper = mount(WorkspaceView, { global: { plugins: [stores] } })
    stores.run.applyEvent(fileChangeEvent(1, 'src/app.ts', 'add'))
    stores.run.applyEvent(fileChangeEvent(2, 'src/app.ts', 'update'))
    await nextTick()

    // The strip's inspector tab is what names this surface now that there is no
    // right column; choosing it opens the panel on the working copy's changes.
    // The rows themselves are `ChangeInspector`'s own contract.
    await wrapper.get('[data-tabstrip-tab="inspector"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-bottom-panel]').attributes('data-bottom-panel-state')).toBe('expanded')
    expect(wrapper.get('[data-bottom-panel-surface]').attributes('data-bottom-panel-surface')).toBe(
      'changes',
    )
  })

  it('sends the model chosen in the composer to the runtime', async () => {
    const chosen = { ...MODEL_CATALOG_FIXTURE, selected_provider: 'openai' }
    const http = {
      get: vi.fn(async (path: string) =>
        path === '/models' ? MODEL_CATALOG_FIXTURE : {},
      ),
      put: vi.fn(async () => chosen),
    } as unknown as HttpClient
    const stores = createAppStores({ http })
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
    await wrapper.get('[data-model-picker] [aria-haspopup="menu"]').trigger('click')
    await nextTick()
    const provider = wrapper
      .findAll('[role="menuitemradio"]')
      .find((item) => item.text().includes('OpenAI'))
    await provider?.trigger('click')
    await flushPromises()

    // The chain the reader's click travels: the picker reports the intent, the
    // panel and the view forward it, and the store asks the runtime - which is
    // what makes the control a control rather than a readout.
    expect(http.put).toHaveBeenCalledWith('/models/selection', {
      provider: 'openai',
      effort: 'high',
    })
    expect(stores.models.catalog?.selected_provider).toBe('openai')
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
