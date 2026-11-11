import codexDark from './assets/agents/codex.png?url'
import codexLight from './assets/agents/codex-light.svg?url'
import claude from './assets/agents/claude.png?url'
import deepseekDark from './assets/agents/deepseek-harness-dark.svg?url'
import deepseekLight from './assets/agents/deepseek-harness-light.svg?url'
import opencode from './assets/agents/opencode.svg?url'
import gemini from './assets/agents/gemini.png?url'
import antigravity from './assets/agents/antigravity.svg?url'
import cursorDark from './assets/agents/cursor-dark.svg?url'
import cursorLight from './assets/agents/cursor-light.svg?url'
import workbuddy from './assets/agents/workbuddy.svg?url'
import qwen from './assets/agents/qwen.svg?url'
import openclaw from './assets/agents/openclaw.svg?url'
import clineDark from './assets/agents/cline-dark.svg?url'
import clineLight from './assets/agents/cline-light.svg?url'
import roo from './assets/agents/roo.png?url'
import kilo from './assets/agents/kilo.svg?url'
import hermesDark from './assets/agents/hermes-dark.svg?url'
import hermesLight from './assets/agents/hermes-light.svg?url'
import pi from './assets/agents/pi.svg?url'
import crush from './assets/agents/crush.png?url'
import trae from './assets/agents/trae.png?url'
import zcode from './assets/agents/zcode.png?url'
import craft from './assets/agents/craft.svg?url'

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
  deepseek: { key: 'deepseek', displayName: 'DeepSeek Harness', label: 'DeepSeek', tone: 'info', marks: { dark: deepseekDark, light: deepseekLight } },
  opencode: { key: 'opencode', displayName: 'OpenCode', label: 'OpenCode', tone: 'accent', marks: { dark: opencode, light: opencode } },
  gemini: { key: 'gemini', displayName: 'Gemini CLI', label: 'Google', tone: 'info', marks: { dark: gemini, light: gemini } },
  antigravity: { key: 'antigravity', displayName: 'Antigravity', label: 'Google', tone: 'accent', marks: { dark: antigravity, light: antigravity } },
  cursor: { key: 'cursor', displayName: 'Cursor', label: 'Cursor', tone: 'neutral', marks: { dark: cursorDark, light: cursorLight } },
  workbuddy: { key: 'workbuddy', displayName: 'WorkBuddy', label: 'Tencent', tone: 'accent', marks: { dark: workbuddy, light: workbuddy } },
  qwen: { key: 'qwen', displayName: 'Qwen Code', label: 'Qwen', tone: 'info', marks: { dark: qwen, light: qwen } },
  openclaw: { key: 'openclaw', displayName: 'OpenClaw', label: 'OpenClaw', tone: 'accent', marks: { dark: openclaw, light: openclaw } },
  cline: { key: 'cline', displayName: 'Cline', label: 'Cline', tone: 'accent', marks: { dark: clineDark, light: clineLight } },
  roo: { key: 'roo', displayName: 'Roo Code', label: 'Roo Code', tone: 'accent', marks: { dark: roo, light: roo } },
  kilo: { key: 'kilo', displayName: 'Kilo Code', label: 'Kilo', tone: 'warning', marks: { dark: kilo, light: kilo } },
  hermes: { key: 'hermes', displayName: 'Hermes Agent', label: 'Nous Research', tone: 'info', marks: { dark: hermesDark, light: hermesLight } },
  pi: { key: 'pi', displayName: 'Pi', label: 'Pi', tone: 'accent', marks: { dark: pi, light: pi } },
  crush: { key: 'crush', displayName: 'Crush', label: 'Charm', tone: 'accent', marks: { dark: crush, light: crush } },
  claude_app: { key: 'claude_app', displayName: 'Claude App', label: 'Anthropic', tone: 'warning', marks: { dark: claude, light: claude } },
  trae: { key: 'trae', displayName: 'Trae', label: 'Trae', tone: 'info', marks: { dark: trae, light: trae } },
  zcode: { key: 'zcode', displayName: 'ZCode', label: 'Z.ai', tone: 'accent', marks: { dark: zcode, light: zcode } },
  craft: { key: 'craft', displayName: 'Craft Agents', label: 'Craft', tone: 'accent', marks: { dark: craft, light: craft } },
} as const satisfies Record<string, AgentIdentity>

export type AgentIdentityKey = keyof typeof AGENT_IDENTITIES

export function agentIdentity(value: string): typeof AGENT_IDENTITIES[AgentIdentityKey] | null {
  const candidate = value.trim().toLowerCase()
  const key = candidate === 'openai' ? 'codex' : candidate === 'anthropic' ? 'claude' : candidate
  return Object.prototype.hasOwnProperty.call(AGENT_IDENTITIES, key)
    ? AGENT_IDENTITIES[key as AgentIdentityKey]
    : null
}
