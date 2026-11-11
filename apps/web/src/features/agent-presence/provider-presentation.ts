import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'

export type AgentProviderKey = 'codex' | 'claude' | 'deepseek' | 'opencode' | 'gemini' | 'antigravity' | 'cursor' | 'workbuddy' | 'qwen' | 'openclaw' | 'generic'
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
  gemini: { key: 'gemini', label: 'Google', iconKey: 'gemini', tone: 'info' },
  antigravity: { key: 'antigravity', label: 'Google', iconKey: 'antigravity', tone: 'accent' },
  cursor: { key: 'cursor', label: 'Cursor', iconKey: 'cursor', tone: 'neutral' },
  workbuddy: { key: 'workbuddy', label: 'Tencent', iconKey: 'workbuddy', tone: 'accent' },
  qwen: { key: 'qwen', label: 'Qwen', iconKey: 'qwen', tone: 'info' },
  openclaw: { key: 'openclaw', label: 'OpenClaw', iconKey: 'openclaw', tone: 'accent' },
}

function knownProviderKey(value: string): Exclude<AgentProviderKey, 'generic'> | null {
  const key = value.trim().toLowerCase()
  if (key === 'openai') return 'codex'
  if (key === 'anthropic') return 'claude'
  return Object.prototype.hasOwnProperty.call(PROVIDERS, key)
    ? key as Exclude<AgentProviderKey, 'generic'>
    : null
}

function knownProvider(agent: AgentRuntimeSummaryDto): Exclude<AgentProviderKey, 'generic'> | null {
  return knownProviderKey(agent.provider) ?? knownProviderKey(agent.icon_key)
}

function humanizeProvider(provider: string): string {
  const text = provider.trim().replace(/[_-]+/g, ' ').replace(/\s+/g, ' ')
  return text.length === 0 ? 'Custom provider' : text.charAt(0).toUpperCase() + text.slice(1)
}

export function agentProviderPresentation(
  agent: AgentRuntimeSummaryDto,
): AgentProviderPresentation {
  const key = knownProvider(agent)
  return key
    ? { ...PROVIDERS[key] }
    : {
        key: 'generic',
        label: humanizeProvider(agent.provider),
        iconKey: 'generic',
        tone: 'neutral',
      }
}
