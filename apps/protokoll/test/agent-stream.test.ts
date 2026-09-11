import { describe, expect, it } from 'vitest'

import {
  parseAgentFleetStreamFrame,
  parseAgentFleetStreamFrameJson,
  type AgentFleetStreamFrameDto,
} from '../src/index'
import { AGENT_FLEET_FIXTURE } from '../src/fixtures/agents'

describe('agent fleet stream frames', () => {
  it('parses a complete snapshot frame through the fleet guard', () => {
    const frame: AgentFleetStreamFrameDto = {
      type: 'snapshot',
      snapshot: AGENT_FLEET_FIXTURE,
    }
    expect(parseAgentFleetStreamFrame(frame)).toEqual(frame)
  })

  it('parses heartbeat frames and rejects unknown fields', () => {
    expect(
      parseAgentFleetStreamFrameJson(
        '{"type":"heartbeat","sequence":12,"sent_at":"2026-08-20T08:10:00.000Z"}',
      ),
    ).toEqual({
      type: 'heartbeat',
      sequence: 12,
      sent_at: '2026-08-20T08:10:00.000Z',
    })
    expect(
      parseAgentFleetStreamFrame({
        type: 'heartbeat',
        sequence: 12,
        sent_at: '2026-08-20T08:10:00.000Z',
        extra: true,
      }),
    ).toBeNull()
  })
})
