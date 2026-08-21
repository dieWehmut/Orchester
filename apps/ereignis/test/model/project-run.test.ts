import {
  eventId,
  fixtureEnvelope,
  runId,
  type RunSnapshotDto,
  type UiEventEnvelope,
} from '@orchester/protokoll'
import { describe, expect, it } from 'vitest'

import { projectRunEvents, projectRunSnapshot } from '../../src/model/project-run'

describe('run event projection', () => {
  it('deduplicates a replayed run sequence and keeps the first observed event', () => {
    const first = fixtureEnvelope(1, { type: 'run_started', title: 'First title' })
    const replay = {
      ...first,
      event_id: eventId('event-replayed-1'),
      kind: { type: 'run_started', title: 'Replayed title' },
    } satisfies UiEventEnvelope

    const view = projectRunEvents([
      first,
      replay,
      fixtureEnvelope(2, { type: 'turn_started' }),
    ])

    expect(view.title).toBe('First title')
    expect(view.status).toBe('running')
    expect(view.latestSequence).toBe(2)
    expect(view.bufferedSequences).toEqual([])
    expect(view.gaps).toEqual([])
  })

  it('projects a contiguous run deterministically when events arrive out of order', () => {
    const events = [
      fixtureEnvelope(3, { type: 'message', text: 'Complete' }),
      fixtureEnvelope(1, { type: 'run_started', title: 'Out of order' }),
      fixtureEnvelope(2, { type: 'turn_started' }),
    ]

    expect(projectRunEvents(events)).toEqual(
      projectRunEvents([...events].sort((left, right) => left.sequence - right.sequence)),
    )
    expect(projectRunEvents(events)).toMatchObject({
      title: 'Out of order',
      latestSequence: 3,
      bufferedSequences: [],
      gaps: [],
    })
  })

  it('buffers events beyond missing ranges and emits deterministic gap markers', () => {
    const view = projectRunEvents([
      fixtureEnvelope(6, { type: 'message', text: 'Still buffered' }),
      fixtureEnvelope(1, { type: 'run_started', title: 'Has gaps' }),
      fixtureEnvelope(4, { type: 'message', text: 'Buffered' }),
    ])

    expect(view.latestSequence).toBe(1)
    expect(view.bufferedSequences).toEqual([4, 6])
    expect(view.gaps).toEqual([
      { key: 'gap:run-fixture:2-3', from: 2, to: 3 },
      { key: 'gap:run-fixture:5-5', from: 5, to: 5 },
    ])
    expect(view.timeline).toEqual([
      {
        type: 'gap',
        key: 'gap:run-fixture:2-3',
        sequence: 2,
        occurredAt: null,
        turnId: null,
        missingFrom: 2,
        missingTo: 3,
      },
      {
        type: 'gap',
        key: 'gap:run-fixture:5-5',
        sequence: 5,
        occurredAt: null,
        turnId: null,
        missingFrom: 5,
        missingTo: 5,
      },
    ])
  })

  it('replaces the previous journal when projecting a bounded snapshot', () => {
    const snapshot: RunSnapshotDto = {
      run_id: runId('run-snapshot'),
      state: 'succeeded',
      events: [
        {
          ...fixtureEnvelope(4, { type: 'run_started', title: 'Fresh snapshot' }),
          run_id: runId('run-snapshot'),
        },
        {
          ...fixtureEnvelope(5, { type: 'run_stopped', reason: 'succeeded' }),
          run_id: runId('run-snapshot'),
        },
      ],
      pending_approvals: [],
      oldest_sequence: 4,
      latest_sequence: 5,
      next_sequence: 6,
      updated_at: '2026-08-19T00:00:05.000Z',
    }

    const view = projectRunSnapshot(snapshot)

    expect(view.runId).toBe(snapshot.run_id)
    expect(view.title).toBe('Fresh snapshot')
    expect(view.status).toBe('succeeded')
    expect(view.latestSequence).toBe(5)
    expect(view.bufferedSequences).toEqual([])
    expect(view.gaps).toEqual([])
  })

  it('retains holes inside a snapshot and adds a trailing head gap', () => {
    const snapshot: RunSnapshotDto = {
      run_id: runId('run-snapshot'),
      state: 'running',
      events: [
        {
          ...fixtureEnvelope(4, { type: 'run_started', title: 'Partial snapshot' }),
          run_id: runId('run-snapshot'),
        },
        {
          ...fixtureEnvelope(6, { type: 'message', text: 'buffered' }),
          run_id: runId('run-snapshot'),
        },
      ],
      pending_approvals: [],
      oldest_sequence: 4,
      latest_sequence: 8,
      next_sequence: 9,
      updated_at: '2026-08-19T00:00:08.000Z',
    }

    expect(projectRunSnapshot(snapshot)).toMatchObject({
      latestSequence: 4,
      bufferedSequences: [6],
      gaps: [
        { key: 'gap:run-snapshot:5-5', from: 5, to: 5 },
        { key: 'gap:run-snapshot:7-8', from: 7, to: 8 },
      ],
    })
  })

  it('rejects a snapshot whose event belongs to another run', () => {
    const snapshot: RunSnapshotDto = {
      run_id: runId('run-snapshot'),
      state: 'running',
      events: [fixtureEnvelope(1, { type: 'run_started' })],
      pending_approvals: [],
      oldest_sequence: 1,
      latest_sequence: 1,
      next_sequence: 2,
      updated_at: '2026-08-19T00:00:01.000Z',
    }

    expect(() => projectRunSnapshot(snapshot)).toThrow(RangeError)
  })
})
