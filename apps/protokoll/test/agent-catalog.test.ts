import { describe, expect, it } from 'vitest'

import {
  AGENT_CATALOG_SCHEMA_VERSION,
  type AgentCatalogDto,
} from '../src/index'

describe('agent catalog DTO', () => {
  it('models capability and tri-state availability without local paths', () => {
    const catalog: AgentCatalogDto = {
      schema_version: AGENT_CATALOG_SCHEMA_VERSION,
      agents: [
        {
          id: 'codex',
          name: 'codex',
          task_kinds: ['code', 'review', 'chat'],
          supports_resume: true,
          streaming: true,
          availability: 'available',
        },
        {
          id: 'optional-agent',
          name: 'optional-agent',
          task_kinds: ['custom:research'],
          supports_resume: false,
          streaming: false,
          availability: 'unknown',
        },
      ],
    }

    expect(catalog.schema_version).toBe(1)
    expect(catalog.agents[0]?.availability).toBe('available')
    expect(catalog.agents[1]?.task_kinds).toEqual(['custom:research'])
    expect(JSON.stringify(catalog)).not.toMatch(/[A-Z]:\\|\/home\//)
  })
})
