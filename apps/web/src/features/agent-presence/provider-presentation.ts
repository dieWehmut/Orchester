import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'

export type AgentProviderKey = 'codex' | 'claude' | 'deepseek' | 'opencode' | 'generic'
export type AgentProviderTone = 'success' | 'warning' | 'info' | 'accent' | 'neutral'

export interface AgentProviderPresentation {
  readonly key: AgentProviderKey
  readonly label: string
  readonly iconKey: AgentProviderKey
  readonly tone: AgentProviderTone
}

const PROVIDERS: Record<Exclude<AgentProviderKey, 'generic'>, AgentProviderPresentation> = {
  codex: { key: 'codex', label: 'OpenAI', iconKey: 'codex', tone: 'success' },
  claude: { key: 'claude', label: 'Anthropic', iconKey: 'claude', tone: 'warning' },
  deepseek: { key: 'deepseek', label: 'DeepSeek', iconKey: 'deepseek', tone: 'info' },
  opencode: { key: 'opencode', label: 'OpenCode', iconKey: 'opencode', tone: 'accent' },
}

function knownProvider(
  agent: AgentRuntimeSummaryDto,
): Exclude<AgentProviderKey, 'generic'> | null {
  const candidates = [agent.provider, agent.icon_key].map((value) => value.trim().toLowerCase())
  if (candidates.some((value) => value === 'codex' || value === 'openai')) return 'codex'
  if (candidates.some((value) => value === 'claude' || value === 'anthropic')) return 'claude'
  if (candidates.includes('deepseek')) return 'deepseek'
  if (candidates.includes('opencode')) return 'opencode'
  return null
}

function humanizeProvider(provider: string): string {
  const text = provider.trim().replace(/[_-]+/g, ' ').replace(/\s+/g, ' ')
  return text.length === 0 ? 'Custom provider' : text.charAt(0).toUpperCase() + text.slice(1)
}

function iconKeyForAgent(agent: AgentRuntimeSummaryDto): AgentProviderKey {
  const iconKey = agent.icon_key.trim().toLowerCase()
  return iconKey === 'codex' || iconKey === 'claude' || iconKey === 'deepseek' || iconKey === 'opencode'
    ? iconKey
    : 'generic'
}

export function agentProviderPresentation(
  agent: AgentRuntimeSummaryDto,
): AgentProviderPresentation {
  const key = knownProvider(agent)
  return key
    ? { ...PROVIDERS[key], iconKey: iconKeyForAgent(agent) }
    : {
        key: 'generic',
        label: humanizeProvider(agent.provider),
        iconKey: 'generic',
        tone: 'neutral',
      }
}
