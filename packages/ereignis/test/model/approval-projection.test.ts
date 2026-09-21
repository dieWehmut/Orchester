import { approvalId, fixtureEnvelope } from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { projectRunEvents } from '../../src/model/project-run'

/**
 * The approval projection, task E1-012.
 *
 * Section 4.7 gives the inspector an Approvals queue whose entries each carry a
 * risk summary, the redacted action and the exact scope, and which renders a
 * stale entry as its own "superseded" state rather than as an error. All of
 * that is a reading of the two approval events the runtime already emits, so
 * the queue's data is projected here rather than re-derived by the component.
 *
 * A resolution is keyed by the same approval id as its request, so an entry is
 * one row that moves from pending to decided rather than two rows that have to
 * be correlated by the reader.
 */

const REQUEST = {
  type: 'approval_requested' as const,
  approval: {
    approval_id: approvalId('approval-1'),
    run_id: fixtureEnvelope(1, { type: 'turn_started' }).run_id,
    row_version: 1,
    risk: 'high',
    action: 'write_file path=src/main.rs',
    reason: 'workspace write',
  },
}

describe('approval projection', () => {
  it('projects a request as a pending entry carrying its scope and risk', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'run_started', title: 'Approve the write' }),
      fixtureEnvelope(2, REQUEST),
    ])

    expect(view.approvals).toHaveLength(1)
    expect(view.approvals[0]).toMatchObject({
      approvalId: 'approval-1',
      rowVersion: 1,
      risk: 'high',
      action: 'write_file path=src/main.rs',
      reason: 'workspace write',
      state: 'pending',
      requestedSequence: 2,
      resolvedSequence: null,
    })
  })

  it('moves the same entry to its decision rather than adding a second row', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, REQUEST),
      fixtureEnvelope(3, {
        type: 'approval_resolved',
        resolution: {
          approval_id: approvalId('approval-1'),
          row_version: 2,
          decision: 'approved',
        },
      }),
    ])

    expect(view.approvals).toHaveLength(1)
    expect(view.approvals[0]).toMatchObject({
      approvalId: 'approval-1',
      rowVersion: 2,
      state: 'approved',
      requestedSequence: 2,
      resolvedSequence: 3,
    })
  })

  it('keeps a superseded entry distinct from a denial', () => {
    // Section 4.7 asks for a distinct superseded state: a row-version mismatch
    // means the entry the reader is looking at is no longer the one the runtime
    // holds, which is not the same news as "denied".
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, REQUEST),
      fixtureEnvelope(3, {
        type: 'approval_resolved',
        resolution: {
          approval_id: approvalId('approval-1'),
          row_version: 2,
          decision: 'stale',
        },
      }),
    ])

    expect(view.approvals[0]?.state).toBe('stale')
  })

  it('keeps two approvals apart', () => {
    const view = projectRunEvents([
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, REQUEST),
      fixtureEnvelope(3, {
        type: 'approval_requested',
        approval: {
          ...REQUEST.approval,
          approval_id: approvalId('approval-2'),
          row_version: 1,
          risk: 'low',
          action: 'read_file path=README.md',
        },
      }),
    ])

    expect(view.approvals.map((entry) => entry.approvalId)).toEqual([
      'approval-1',
      'approval-2',
    ])
  })

  it('has no approvals before any is requested', () => {
    expect(
      projectRunEvents([fixtureEnvelope(1, { type: 'run_started' })]).approvals,
    ).toEqual([])
  })
})
