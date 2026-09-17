import { describe, expect, it } from 'vitest'

import {
  AGENT_STATUS_SCHEMA_VERSION,
  isAgentFleetSnapshot,
  parseAgentFleetSnapshot,
  type AgentFleetSnapshotDto,
} from '../src/index'

const baseAgent = {
  agent_id: 'codex-main',
  provider: 'codex',
  display_name: 'Codex',
  icon_key: 'codex',
  availability: 'available' as const,
  activity: 'running' as const,
  installed: true,
  configured: true,
  authenticated: true,
  active_windows: 2,
  active_sessions: 3,
  active_runs: 2,
  active_subagents: 1,
  window_count_source: 'managed_sessions' as const,
  last_heartbeat_at: '2026-08-20T08:00:00.000Z',
  last_error: null,
  capabilities: ['streaming', 'resume'],
  updated_at: '2026-08-20T08:00:01.000Z',
}

const snapshot: AgentFleetSnapshotDto = {
  schema_version: AGENT_STATUS_SCHEMA_VERSION,
  sequence: 7,
  generated_at: '2026-08-20T08:00:02.000Z',
  agents: [baseAgent],
}

describe('agent fleet status DTO', () => {
  it('accepts independent availability/activity and managed window counts', () => {
    const parsed = parseAgentFleetSnapshot(snapshot)

    expect(parsed).toEqual(snapshot)
    expect(parsed?.agents[0]).toMatchObject({
      availability: 'available',
      activity: 'running',
      active_windows: 2,
      active_subagents: 1,
      window_count_source: 'managed_sessions',
    })
    expect(isAgentFleetSnapshot(parsed)).toBe(true)
  })

  it('accepts aggregated external agent process instances', () => {
    const parsed = parseAgentFleetSnapshot({
      ...snapshot,
      schema_version: 2,
      agents: [
        {
          ...baseAgent,
          active_windows: 3,
          window_count_source: 'external_processes',
        },
      ],
    })

    expect(parsed?.agents[0]).toMatchObject({
      active_windows: 3,
      window_count_source: 'external_processes',
    })
  })

  it('keeps an unavailable provider in the snapshot without inventing activity', () => {
    const parsed = parseAgentFleetSnapshot({
      ...snapshot,
      agents: [
        {
          ...baseAgent,
          agent_id: 'claude-local',
          provider: 'claude',
          display_name: 'Claude Code',
          icon_key: 'claude',
          availability: 'auth_required',
          activity: 'offline',
          installed: true,
          configured: false,
          authenticated: false,
          active_windows: 0,
          active_sessions: 0,
          active_runs: 0,
          active_subagents: 0,
          window_count_source: 'managed_sessions',
          last_error: 'Authentication is required',
        },
      ],
    })

    expect(parsed?.agents[0]).toMatchObject({
      availability: 'auth_required',
      activity: 'offline',
      active_runs: 0,
    })
  })

  it('rejects unknown fields and sensitive path-shaped errors', () => {
    expect(parseAgentFleetSnapshot({ ...snapshot, unexpected: true })).toBeNull()
    expect(
      parseAgentFleetSnapshot({
        ...snapshot,
        agents: [{ ...baseAgent, last_error: 'failed at C:\\Users\\dev\\transcript.json' }],
      }),
    ).toBeNull()
  })

  it('rejects duplicate agents, invalid counts, and invalid icon keys', () => {
    expect(parseAgentFleetSnapshot({ ...snapshot, agents: [baseAgent, baseAgent] })).toBeNull()
    expect(
      parseAgentFleetSnapshot({
        ...snapshot,
        agents: [{ ...baseAgent, active_windows: -1 }],
      }),
    ).toBeNull()
    expect(
      parseAgentFleetSnapshot({
        ...snapshot,
        agents: [{ ...baseAgent, icon_key: 'claude/../../secret' }],
      }),
    ).toBeNull()
    expect(
      parseAgentFleetSnapshot({
        ...snapshot,
        agents: [{ ...baseAgent, window_count_source: 'provider_windows' }],
      }),
    ).toBeNull()
  })
})
