import { describe, expect, it } from 'vitest'

import { approvalId, fixtureEnvelope, runId } from '@orchester/protokoll'

import {
  ANNOUNCEMENT_POLITENESS,
  advanceAnnouncements,
  announcementFor,
  cursorAtHead,
  EMPTY_ANNOUNCEMENT_CURSOR,
} from '../src/components/run/announcements'

/**
 * The live-region half of the accessibility contract.
 *
 * Section 7 asks for three things that are easy to get backwards:
 *
 * - run status, approval arrival and validator results are `polite`;
 * - failures are `assertive`;
 * - streaming tokens are *not* announced, and the turn is announced once when
 *   it completes.
 *
 * The last one is the reason this is a function over named triggers rather than
 * a live region on the transcript: a region on the transcript would read every
 * token as it arrived, because a token is a text change like any other. A
 * trigger has to name the change, and a streamed chunk is not one of the names.
 */

describe('run announcements', () => {
  it('announces the start of a run politely, once', () => {
    expect(ANNOUNCEMENT_POLITENESS.status).toBe('polite')
    expect(announcementFor({ kind: 'status', status: 'running', previousStatus: 'idle' })).toEqual({
      kind: 'status',
      politeness: 'polite',
      key: 'run.announce.started',
    })
    expect(
      announcementFor({ kind: 'status', status: 'running', previousStatus: 'running' }),
    ).toBeNull()
  })

  it('escalates a terminal failure to an assertive announcement', () => {
    expect(announcementFor({ kind: 'status', status: 'failed', previousStatus: 'running' })).toEqual({
      kind: 'status',
      politeness: 'assertive',
      key: 'run.announce.failed',
    })
    for (const status of ['budget_exceeded', 'repeated_failure'] as const) {
      expect(
        announcementFor({ kind: 'status', status, previousStatus: 'running' })?.politeness,
        status,
      ).toBe('assertive')
    }
  })

  it('keeps the quiet outcomes polite', () => {
    expect(announcementFor({ kind: 'status', status: 'succeeded', previousStatus: 'running' })).toEqual({
      kind: 'status',
      politeness: 'polite',
      key: 'run.announce.succeeded',
    })
    expect(
      announcementFor({ kind: 'status', status: 'cancelled', previousStatus: 'running' })?.politeness,
    ).toBe('polite')
    expect(
      announcementFor({ kind: 'status', status: 'interrupted_unknown_outcome', previousStatus: 'running' }),
    ).toEqual({
      kind: 'status',
      politeness: 'assertive',
      key: 'run.announce.interrupted',
    })
  })

  it('leaves the approval pause to the approval arrival', () => {
    // The approval arrival is the news; the pause that follows it would repeat
    // it, so the status has no sentence of its own.
    expect(
      announcementFor({ kind: 'status', status: 'awaiting_approval', previousStatus: 'running' }),
    ).toBeNull()
    expect(announcementFor({ kind: 'approval' })).toEqual({
      kind: 'approval',
      politeness: 'polite',
      key: 'run.announce.approval',
    })
  })

  it('announces the completed turn politely', () => {
    expect(announcementFor({ kind: 'turn', turnId: 'turn-1' })).toEqual({
      kind: 'turn',
      politeness: 'polite',
      key: 'run.announce.turnComplete',
    })
  })

  it('announces validator results with the summary, assertively when failed', () => {
    expect(
      announcementFor({ kind: 'validation', validation: { ok: true, summary: 'All checks passed' } }),
    ).toEqual({
      kind: 'validation',
      politeness: 'polite',
      key: 'run.announce.validation',
      params: { summary: 'All checks passed' },
    })
    expect(
      announcementFor({ kind: 'validation', validation: { ok: false, summary: '2 of 5 checks failed' } }),
    ).toEqual({
      kind: 'validation',
      politeness: 'assertive',
      key: 'run.announce.validationFailed',
      params: { summary: '2 of 5 checks failed' },
    })
  })
})

describe('journal announcements', () => {
  it('says nothing at all about a journal a reader has already seen', () => {
    const events = [
      fixtureEnvelope(1, { type: 'run_started', title: 'Inspect' }),
      fixtureEnvelope(2, { type: 'turn_started' }),
      fixtureEnvelope(3, { type: 'message', text: 'Done.' }),
      fixtureEnvelope(4, { type: 'run_stopped', reason: 'succeeded' }),
    ]

    const { announcements } = advanceAnnouncements(cursorAtHead(events), events)

    // The retained window is history, not news: reading the transcript that was
    // already on screen must not make the screen reader narrate it.
    expect(announcements).toEqual([])
  })

  it('speaks the run lifecycle, the turn, the approval and the validation in order', () => {
    const events = [
      fixtureEnvelope(1, { type: 'run_started', title: 'Inspect' }),
      fixtureEnvelope(2, { type: 'turn_started' }),
      fixtureEnvelope(3, { type: 'message_delta', text: 'Work', final: false }),
      fixtureEnvelope(4, { type: 'message_delta', text: 'ing', final: true }),
      fixtureEnvelope(5, {
        type: 'approval_requested',
        approval: {
          approval_id: approvalId('approval-1'),
          run_id: runId('run-fixture'),
          row_version: 1,
          risk: 'workspace_write',
          action: 'write_file path=src/main.ts',
          reason: 'The action modifies source code',
        },
      }),
      fixtureEnvelope(6, { type: 'run_stopped', reason: 'awaiting_approval' }),
      fixtureEnvelope(7, { type: 'validation', validation: { ok: true, summary: 'Checks passed' } }),
      fixtureEnvelope(8, { type: 'run_stopped', reason: 'succeeded' }),
    ]
    const { announcements } = advanceAnnouncements(EMPTY_ANNOUNCEMENT_CURSOR, events)

    expect(announcements.map((entry) => entry.key)).toEqual([
      'run.announce.started',
      'run.announce.turnComplete',
      'run.announce.approval',
      'run.announce.validation',
      'run.announce.succeeded',
    ])
  })

  it('never announces the streamed chunks, only the completed turn', () => {
    const events = [
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, { type: 'message_delta', text: 'a', final: false }),
      fixtureEnvelope(3, { type: 'message_delta', text: 'b', final: false }),
      fixtureEnvelope(4, { type: 'message_delta', text: 'c', final: false }),
    ]

    const { announcements } = advanceAnnouncements(EMPTY_ANNOUNCEMENT_CURSOR, events)

    expect(announcements.map((entry) => entry.key)).toEqual(['run.announce.started'])
  })

  it('announces a turn once even when the stream and the whole message both land', () => {
    const events = [
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, { type: 'message_delta', text: 'Done', final: true }),
      fixtureEnvelope(3, { type: 'message', text: 'Done' }),
    ]

    const { announcements } = advanceAnnouncements(EMPTY_ANNOUNCEMENT_CURSOR, events)

    expect(announcements.filter((entry) => entry.kind === 'turn')).toHaveLength(1)
  })

  it('only offers the events past the cursor, in order', () => {
    const events = [
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, { type: 'message', text: 'First reply' }),
      fixtureEnvelope(3, { type: 'message', text: 'Second reply' }),
    ]

    const first = advanceAnnouncements(EMPTY_ANNOUNCEMENT_CURSOR, events)
    const second = advanceAnnouncements(first.cursor, events)

    expect(first.announcements.map((entry) => entry.key)).toEqual([
      'run.announce.started',
      'run.announce.turnComplete',
    ])
    // Re-advancing over the same journal is not news again.
    expect(second.announcements).toEqual([])
  })

  it('starts over for a new run and stays quiet across a snapshot rewind', () => {
    const first = advanceAnnouncements(EMPTY_ANNOUNCEMENT_CURSOR, [
      fixtureEnvelope(1, { type: 'run_started' }),
      fixtureEnvelope(2, { type: 'message', text: 'Done' }),
    ])
    const otherRun = [
      { ...fixtureEnvelope(1, { type: 'run_started' }), run_id: runId('run-other') },
      { ...fixtureEnvelope(2, { type: 'message', text: 'Fresh' }), run_id: runId('run-other') },
    ]
    const restarted = advanceAnnouncements(first.cursor, otherRun)

    expect(restarted.announcements.map((entry) => entry.key)).toEqual([
      'run.announce.started',
      'run.announce.turnComplete',
    ])

    // Same run, but the journal rewound to an older retained window: that is a
    // snapshot replacing what was on screen, and the reader is not re-read to.
    const rewound = advanceAnnouncements(restarted.cursor, otherRun.slice(0, 1))
    expect(rewound.announcements).toEqual([])
  })
})
