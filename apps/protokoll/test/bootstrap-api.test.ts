import { describe, expect, it } from 'vitest'

import {
  BOOTSTRAP_SCHEMA_VERSION,
  type BootstrapDto,
} from '../src/api'

describe('bootstrap contract', () => {
  it('describes server and workspace state without an absolute path', () => {
    const response: BootstrapDto = {
      schema_version: BOOTSTRAP_SCHEMA_VERSION,
      service_version: '0.1.2',
      server_state: 'running',
      workspace: {
        selected: true,
        name: 'Orchester',
      },
    }

    expect(response.workspace).toEqual({ selected: true, name: 'Orchester' })
    expect(response).not.toHaveProperty('workspace_path')
  })
})
