import { describe, expect, it } from 'vitest'

import {
  AGENT_FLEET_FIXTURE,
  parseAgentFleetSnapshot,
} from '../../src/index'

describe('agent fleet fixture', () => {
  it('covers Codex, Claude Code, DeepSeek, unavailable, and auth-required states', () => {
    const parsed = parseAgentFleetSnapshot(AGENT_FLEET_FIXTURE)

    expect(parsed).not.toBeNull()
    expect(parsed?.agents.map((agent) => agent.provider)).toEqual([
      'codex',
      'claude',
      'deepseek',
      'opencode',
      'claude',
    ])
    expect(parsed?.agents.map((agent) => agent.availability)).toEqual([
      'available',
      'available',
      'available',
      'unavailable',
      'auth_required',
    ])
    expect(parsed?.agents.find((agent) => agent.provider === 'deepseek')).toMatchObject({
      activity: 'running',
      active_subagents: 2,
    })
  })

  it('contains no local paths or credentials', () => {
    const serialized = JSON.stringify(AGENT_FLEET_FIXTURE)

    expect(serialized).not.toMatch(/[A-Z]:\\|\/(?:home|Users|private|tmp|var)\//i)
    expect(serialized).not.toMatch(/api[_-]?key|password|secret|bearer\s|token[=:]/i)
  })
})
