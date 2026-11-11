import { describe, expect, it } from 'vitest'
import { AGENT_IDENTITIES, agentIdentity } from '../src/agent-identities'

const expected = [
  'codex', 'claude', 'deepseek', 'opencode', 'gemini', 'antigravity', 'cursor',
  'workbuddy', 'workbuddy_global', 'qwen', 'openclaw', 'cline', 'roo', 'kilo', 'hermes', 'pi', 'crush',
  'claude_app', 'trae', 'zcode', 'craft',
]
describe('canonical official agent identities', () => {
  it('covers every documented tool and all existing monitors with bundled official marks', () => {
    expect(Object.keys(AGENT_IDENTITIES).sort()).toEqual([...expected].sort())
    for (const key of expected) {
      const identity = agentIdentity(key)
      expect(identity?.key).toBe(key)
      expect(identity?.marks.dark).toBeTruthy()
      expect(identity?.marks.light).toBeTruthy()
      expect(identity?.marks.dark).not.toMatch(/^\/agents\//)
    }
  })
  it('shares the verified Claude mark across its CLI and desktop identities', () => {
    expect(agentIdentity('claude_app')?.marks).toEqual(agentIdentity('claude')?.marks)
  })
  it('names the two WorkBuddy editions separately with their shared official brand mark', () => {
    expect(agentIdentity('workbuddy')?.displayName).toBe('WorkBuddy 国内版')
    expect(agentIdentity('workbuddy_global')?.displayName).toBe('WorkBuddy AI 国际版')
    expect(agentIdentity('workbuddy_global')?.marks).toEqual(agentIdentity('workbuddy')?.marks)
  })
  it('keeps aliases stable without accepting object prototype names as identities', () => {
    expect(agentIdentity(' OPENAI ')?.key).toBe('codex')
    expect(agentIdentity('ANTHROPIC')?.key).toBe('claude')
    expect(agentIdentity('constructor')).toBeNull()
    expect(agentIdentity('__proto__')).toBeNull()
    expect(agentIdentity('custom-provider')).toBeNull()
  })
})
