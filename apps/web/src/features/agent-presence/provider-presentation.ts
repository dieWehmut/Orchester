import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { agentIdentity, type AgentIdentityKey, type AgentIdentityTone } from '@orchester/design/agent-identities'

export type AgentProviderKey = AgentIdentityKey | 'generic'
export type AgentProviderTone = AgentIdentityTone

export interface AgentProviderPresentation {
  readonly key: AgentProviderKey
  readonly label: string
  readonly iconKey: AgentProviderKey
  readonly tone: AgentProviderTone
}

function humanizeProvider(provider: string): string {
  const text = provider.trim().replace(/[_-]+/g, ' ').replace(/\s+/g, ' ')
  return text.length === 0 ? 'Custom provider' : text.charAt(0).toUpperCase() + text.slice(1)
}

export function agentProviderPresentation(
  agent: AgentRuntimeSummaryDto,
): AgentProviderPresentation {
  const identity = agentIdentity(agent.provider) ?? agentIdentity(agent.icon_key)
  return identity
    ? { key: identity.key, label: identity.label, iconKey: identity.key, tone: identity.tone }
    : {
        key: 'generic',
        label: humanizeProvider(agent.provider),
        iconKey: 'generic',
        tone: 'neutral',
      }
}
