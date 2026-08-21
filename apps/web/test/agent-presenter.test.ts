import { AGENT_FLEET_FIXTURE } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import {
  agentActivityMessageKey,
  agentCountMessageKey,
  agentStreamStatusMessageKey,
  agentWindowSourceMessageKey,
  activeAgentCounts,
} from '../src/features/agent-presence'

describe('agent presenter helpers', () => {
  it('maps availability and activity to stable locale keys', () => {
    const authRequired = AGENT_FLEET_FIXTURE.agents.find((agent) => agent.availability === 'auth_required')
    const running = AGENT_FLEET_FIXTURE.agents.find((agent) => agent.activity === 'running')

    expect(authRequired).toBeDefined()
    expect(running).toBeDefined()
    expect(agentActivityMessageKey(authRequired!)).toBe('agents.activity.signInRequired')
    expect(agentActivityMessageKey(running!)).toBe('agents.activity.running')
  })

  it('maps stream lifecycle states to stable locale keys', () => {
    expect(agentStreamStatusMessageKey('connected')).toBe('agents.stream.live')
    expect(agentStreamStatusMessageKey('reconnecting')).toBe('agents.stream.reconnecting')
    expect(agentStreamStatusMessageKey('fatal')).toBe('agents.stream.offline')
  })

  it('keeps active windows, runs, and subagents as explicit count entries', () => {
    const codex = AGENT_FLEET_FIXTURE.agents[0]
    if (!codex) throw new Error('fixture must contain Codex')

    expect(activeAgentCounts(codex)).toEqual([
      { key: 'windows', count: 2 },
      { key: 'runs', count: 2 },
      { key: 'subagents', count: 1 },
    ])
  })

  it('selects singular metric copy and explains the runtime window source', () => {
    expect(agentCountMessageKey('windows', 1)).toBe('agents.counts.window')
    expect(agentCountMessageKey('runs', 2)).toBe('agents.counts.runs')
    expect(agentCountMessageKey('subagents', 1)).toBe('agents.counts.subagent')
    expect(agentWindowSourceMessageKey('managed_sessions')).toBe(
      'agents.windowSource.managedSessions',
    )
    expect(agentWindowSourceMessageKey('tauri_windows')).toBe(
      'agents.windowSource.desktopWindows',
    )
  })
})
