import { AGENT_FLEET_FIXTURE, type AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { agentProviderPresentation } from '../src/features/agent-presence'

describe('agent provider presentation', () => {
  it.each([
    { key: 'gemini', label: 'Google', tone: 'info' },
    { key: 'antigravity', label: 'Google', tone: 'accent' },
    { key: 'cursor', label: 'Cursor', tone: 'neutral' },
    { key: 'workbuddy', label: 'Tencent', tone: 'accent' },
    { key: 'qwen', label: 'Qwen', tone: 'info' },
    { key: 'openclaw', label: 'OpenClaw', tone: 'accent' },
  ])('identifies $key independently of an unknown runtime icon', ({ key, label, tone }) => {
    const agent: AgentRuntimeSummaryDto = { ...AGENT_FLEET_FIXTURE.agents[0]!, provider: key, icon_key: 'custom-provider' }

    expect(agentProviderPresentation(agent)).toEqual({ key, label, iconKey: key, tone })
  })

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

  it.each(['workbuddy', 'qwen', 'openclaw'])('keeps $0 provider identity when a runtime has an old known icon', (provider) => {
    const agent: AgentRuntimeSummaryDto = { ...AGENT_FLEET_FIXTURE.agents[0]!, provider, icon_key: 'codex' }

    expect(agentProviderPresentation(agent)).toMatchObject({ key: provider, iconKey: provider })
  })

  it.each(['workbuddy', 'qwen', 'openclaw'])('uses $0 as the icon fallback for an unknown provider', (iconKey) => {
    const agent: AgentRuntimeSummaryDto = { ...AGENT_FLEET_FIXTURE.agents[0]!, provider: 'custom-runtime', icon_key: iconKey }

    expect(agentProviderPresentation(agent)).toMatchObject({ key: iconKey, iconKey })
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

  it('uses the official provider identity even when its runtime reports an unknown icon key', () => {
    const customIcon: AgentRuntimeSummaryDto = {
      ...AGENT_FLEET_FIXTURE.agents[0]!,
      icon_key: 'custom-provider',
    }

    expect(agentProviderPresentation(customIcon)).toEqual({
      key: 'codex',
      label: 'OpenAI',
      iconKey: 'codex',
      tone: 'success',
    })
  })
})
