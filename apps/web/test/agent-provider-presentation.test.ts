import { AGENT_FLEET_FIXTURE, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { agentProviderPresentation } from '../src/features/agent-presence'

describe('agent provider presentation', () => {
  it('maps known runtimes to stable provider identities', () => {
    expect(agentProviderPresentation(AGENT_FLEET_FIXTURE.agents[0]!)).toEqual({
      key: 'codex',
      label: 'OpenAI',
      iconKey: 'codex',
      tone: 'success',
    })
    expect(agentProviderPresentation(AGENT_FLEET_FIXTURE.agents[1]!)).toEqual({
      key: 'claude',
      label: 'Anthropic',
      iconKey: 'claude',
      tone: 'warning',
    })
  })

  it('humanizes unknown providers and uses the generic identity', () => {
    const custom: AgentRuntimeSummaryDto = {
      ...AGENT_FLEET_FIXTURE.agents[0]!,
      provider: 'local-bridge',
      icon_key: 'local-bridge',
    }
    expect(agentProviderPresentation(custom)).toEqual({
      key: 'generic',
      label: 'Local bridge',
      iconKey: 'generic',
      tone: 'neutral',
    })
  })

  it('keeps a generic icon when a known provider reports an unknown icon key', () => {
    const customIcon: AgentRuntimeSummaryDto = {
      ...AGENT_FLEET_FIXTURE.agents[0]!,
      icon_key: 'custom-provider',
    }

    expect(agentProviderPresentation(customIcon)).toEqual({
      key: 'codex',
      label: 'OpenAI',
      iconKey: 'generic',
      tone: 'success',
    })
  })
})
