import codexDark from './assets/agents/codex.png?url'
import codexLight from './assets/agents/codex-light.png?url'
import claude from './assets/agents/claude.svg?url'
import deepseek from './assets/agents/deepseek.ico?url'
import opencodeDark from './assets/agents/opencode-dark.svg?url'
import opencodeLight from './assets/agents/opencode-light.svg?url'
import gemini from './assets/agents/gemini.png?url'
import antigravity from './assets/agents/antigravity.svg?url'
import cursorDark from './assets/agents/cursor-dark.svg?url'
import cursorLight from './assets/agents/cursor-light.svg?url'
import workbuddy from './assets/agents/workbuddy.svg?url'
import qwen from './assets/agents/qwen.svg?url'
import openclaw from './assets/agents/openclaw.svg?url'

export type AgentIdentityTone = 'success' | 'warning' | 'info' | 'accent' | 'neutral'
export interface AgentIdentity {
  readonly key: string
  readonly displayName: string
  readonly label: string
  readonly tone: AgentIdentityTone
  readonly marks: { readonly dark: string; readonly light: string }
}

export const AGENT_IDENTITIES = {
  codex: { key: 'codex', displayName: 'Codex', label: 'OpenAI', tone: 'success', marks: { dark: codexDark, light: codexLight } },
  claude: { key: 'claude', displayName: 'Claude Code', label: 'Anthropic', tone: 'warning', marks: { dark: claude, light: claude } },
  deepseek: { key: 'deepseek', displayName: 'DeepSeek Harness', label: 'DeepSeek', tone: 'info', marks: { dark: deepseek, light: deepseek } },
  opencode: { key: 'opencode', displayName: 'OpenCode', label: 'OpenCode', tone: 'accent', marks: { dark: opencodeDark, light: opencodeLight } },
  gemini: { key: 'gemini', displayName: 'Gemini CLI', label: 'Google', tone: 'info', marks: { dark: gemini, light: gemini } },
  antigravity: { key: 'antigravity', displayName: 'Antigravity', label: 'Google', tone: 'accent', marks: { dark: antigravity, light: antigravity } },
  cursor: { key: 'cursor', displayName: 'Cursor', label: 'Cursor', tone: 'neutral', marks: { dark: cursorDark, light: cursorLight } },
  workbuddy: { key: 'workbuddy', displayName: 'WorkBuddy', label: 'Tencent', tone: 'accent', marks: { dark: workbuddy, light: workbuddy } },
  qwen: { key: 'qwen', displayName: 'Qwen Code', label: 'Qwen', tone: 'info', marks: { dark: qwen, light: qwen } },
  openclaw: { key: 'openclaw', displayName: 'OpenClaw', label: 'OpenClaw', tone: 'accent', marks: { dark: openclaw, light: openclaw } },
} as const satisfies Record<string, AgentIdentity>

export type AgentIdentityKey = keyof typeof AGENT_IDENTITIES

export function agentIdentity(value: string): typeof AGENT_IDENTITIES[AgentIdentityKey] | null {
  const candidate = value.trim().toLowerCase()
  const key = candidate === 'openai' ? 'codex' : candidate === 'anthropic' ? 'claude' : candidate
  return Object.prototype.hasOwnProperty.call(AGENT_IDENTITIES, key)
    ? AGENT_IDENTITIES[key as AgentIdentityKey]
    : null
}
