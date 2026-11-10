import { describe, expect, it } from 'vitest'

import { AGENT_FLEET_FIXTURE, isAgentFleetSnapshot, parseAgentFleetSnapshot, parseAgentFleetStreamFrame } from '../src/index'

const observedAt = '2026-11-10T05:21:04.000Z'
const v3Agent = { ...AGENT_FLEET_FIXTURE.agents[0]!, activity: 'unknown', process_sources: null }
const fleet = (agent: unknown) => ({ ...AGENT_FLEET_FIXTURE, schema_version: 3, agents: [agent] })

describe('agent process source protocol', () => {
  it('accepts v3 unknown and independent successful, stale, and unavailable sources', () => {
    const raw = fleet({ ...v3Agent, process_sources: [
      { environment: 'windows', status: 'available', count: 3, observed_at: observedAt },
      { environment: 'wsl', status: 'stale', count: 2, observed_at: observedAt },
      { environment: 'wsl', status: 'unavailable', count: null, observed_at: null },
    ] })
    expect(parseAgentFleetSnapshot(raw)).toEqual(raw)
    expect(parseAgentFleetStreamFrame({ type: 'snapshot', snapshot: raw })).toEqual({ type: 'snapshot', snapshot: raw })
  })

  it('normalizes strict legacy v2 without pretending its raw shape is a v3 DTO', () => {
    const { process_sources: _sources, ...agent } = v3Agent
    const legacy = { ...fleet({ ...agent, activity: 'idle', active_windows: 0 }), schema_version: 2 }
    expect(parseAgentFleetSnapshot(legacy)?.agents[0]?.process_sources).toBeNull()
    expect(parseAgentFleetSnapshot(legacy)?.schema_version).toBe(2)
    expect(isAgentFleetSnapshot(legacy)).toBe(false)
    expect(parseAgentFleetSnapshot({ ...legacy, agents: [{ ...agent, activity: 'unknown' }] })).toBeNull()
    expect(parseAgentFleetSnapshot({ ...legacy, agents: [v3Agent] })).toBeNull()
  })

  it('normalizes strict legacy v2 WebSocket frames with the same rules as HTTP', () => {
    const { process_sources: _sources, ...agent } = v3Agent
    const legacy = { ...fleet({ ...agent, activity: 'idle', active_windows: 0 }), schema_version: 2 }
    expect(parseAgentFleetStreamFrame({ type: 'snapshot', snapshot: legacy })).toEqual({
      type: 'snapshot', snapshot: parseAgentFleetSnapshot(legacy),
    })
    for (const invalid of [
      { ...legacy, agents: [{ ...agent, activity: 'unknown' }] },
      { ...legacy, agents: [{ ...agent, activity: 'idle', process_sources: null }] },
    ]) expect(parseAgentFleetStreamFrame({ type: 'snapshot', snapshot: invalid })).toBeNull()
  })

  it('requires the v3 field and rejects empty or malformed observations', () => {
    const { process_sources: _sources, ...agent } = v3Agent
    expect(parseAgentFleetSnapshot(fleet(agent))).toBeNull()
    expect(parseAgentFleetSnapshot(fleet({ ...v3Agent, process_sources: [] }))).toBeNull()
    const valid = { environment: 'local', status: 'available', count: 0, observed_at: observedAt }
    for (const source of [
      { ...valid, count: -1 }, { ...valid, count: Number.MAX_SAFE_INTEGER + 1 },
      { ...valid, count: null }, { ...valid, observed_at: null },
      { ...valid, observed_at: 'not-a-date' }, { ...valid, environment: 'Ubuntu-private' },
      { ...valid, observed_at: '2026-02-31T05:21:04Z' }, { ...valid, observed_at: '2026-11-10T24:00:00Z' },
      { ...valid, extra: true }, { ...valid, status: 'unavailable' },
      { ...valid, status: 'stale', observed_at: null },
    ]) expect(parseAgentFleetSnapshot(fleet({ ...v3Agent, process_sources: [source] }))).toBeNull()
  })

  it('rejects external current counts inconsistent with successful sources, excluding stale counts', () => {
    const source = { environment: 'windows', status: 'available', count: 3, observed_at: observedAt }
    const stale = { environment: 'wsl', status: 'stale', count: 7, observed_at: observedAt }
    const agent = { ...v3Agent, window_count_source: 'external_processes', active_windows: 3, process_sources: [source, stale] }
    expect(parseAgentFleetSnapshot(fleet(agent))).not.toBeNull()
    expect(parseAgentFleetSnapshot(fleet({ ...agent, active_windows: 10 }))).toBeNull()
    expect(parseAgentFleetSnapshot(fleet({ ...agent, active_windows: 3, process_sources: null }))).not.toBeNull()
  })
})
