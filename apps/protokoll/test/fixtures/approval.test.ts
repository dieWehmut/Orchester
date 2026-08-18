import { describe, expect, it } from 'vitest'

import { approvalPathFixture, parseUiEventEnvelope } from '../../src/index'

describe('approval path fixture', () => {
  it('pauses before resolution and resumes the same tool call afterwards', () => {
    const events = approvalPathFixture()
    const kinds = events.map((event) => event.kind)
    const requested = kinds.find((kind) => kind.type === 'approval_requested')
    const resolved = kinds.find((kind) => kind.type === 'approval_resolved')
    const awaitingIndex = kinds.findIndex(
      (kind) => kind.type === 'run_stopped' && kind.reason === 'awaiting_approval',
    )
    const resolutionIndex = kinds.findIndex((kind) => kind.type === 'approval_resolved')

    expect(awaitingIndex).toBeGreaterThan(-1)
    expect(resolutionIndex).toBeGreaterThan(awaitingIndex)
    expect(requested?.type === 'approval_requested' && requested.approval.approval_id).toBe(
      resolved?.type === 'approval_resolved' ? resolved.resolution.approval_id : undefined,
    )
    expect(events.every((event) => parseUiEventEnvelope(event) !== null)).toBe(true)
  })
})
