import { type BootstrapDto } from '@orchester/protokoll'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createAppStores } from '../src/stores/app'
import WorkspaceView from '../src/views/WorkspaceView.vue'

function readyStores() {
  const stores = createAppStores()
  stores.bootstrap.context.value = {
    schema_version: 1,
    service_version: '0.1.2',
    server_state: 'running',
    workspace: { selected: true, name: 'Orchester' },
  } satisfies BootstrapDto
  stores.bootstrap.status.value = 'ready'
  return stores
}

describe('WorkspaceView greeting', () => {
  it('greets the operator with the Codex-style centered prompt', () => {
    const wrapper = mount(WorkspaceView, { global: { plugins: [readyStores()] } })

    const empty = wrapper.get('[data-run-empty]')
    expect(empty.text()).toContain('What should we build in Orchester?')
    expect(empty.get('[data-orchester-mark]')).toBeTruthy()
  })
})
