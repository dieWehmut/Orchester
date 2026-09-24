import { describe, expect, it } from 'vitest'

import { answerElapsed, formatElapsed, runElapsed } from '../src/components/run/run-duration'
import type { RunView } from '@orchester/ereignis'

/**
 * How long a run took, in the reference's own three shapes.
 *
 * The reference states an elapsed time above its answer and leaves it out while
 * there is nothing to state. This product has one turn per run, so the run's own
 * elapsed time is what can be said - and it is read from the event stream's
 * timestamps, not from whenever the window opened.
 */

function view(occurred: readonly string[], stop: string | null = null): RunView {
  return {
    runId: 'run-1',
    title: null,
    status: 'running',
    stop: stop === null ? null : { reason: 'succeeded', sequence: 9, occurredAt: stop, outcome: 'terminal' },
    turns: [],
    timeline: occurred.map((occurredAt, index) => ({
      key: `k-${index}`,
      sequence: index,
      turnId: null,
      occurredAt,
      type: 'message' as const,
      role: 'assistant' as const,
      text: 'answer',
      final: true,
    })),
    tools: [],
    fileChanges: [],
    validations: [],
    approvals: [],
    todos: [],
    gaps: [],
    latestSequence: 9,
    usage: { input_tokens: 11, output_tokens: 22, cached_input_tokens: 0, reasoning_tokens: 0 },
  } as unknown as RunView
}

describe('formatElapsed', () => {
  it('says seconds while it is quick, minutes while it is not, and hours with the rest padded', () => {
    expect(formatElapsed(0)).toBe('0s')
    expect(formatElapsed(12_000)).toBe('12s')
    expect(formatElapsed(200_000)).toBe('3m 20s')
    expect(formatElapsed(3_663_000)).toBe('1h 01m 03s')
  })

  it('never counts backwards', () => {
    expect(formatElapsed(-5_000)).toBe('0s')
  })
})

describe('runElapsed', () => {
  it('measures the run from its first event to its last', () => {
    const elapsed = runElapsed(
      view(['2026-09-16T10:00:00Z', '2026-09-16T10:03:20Z'], '2026-09-16T10:03:20Z'),
    )

    expect(elapsed).toBe('3m 20s')
  })

  it('takes the stop into account when it is later than the last event', () => {
    expect(runElapsed(view(['2026-09-16T10:00:00Z'], '2026-09-16T10:33:20Z'))).toBe('33m 20s')
  })

  it('says nothing about a run that has not taken a measurable moment', () => {
    // "0s" beside a run in flight is a claim the stream does not support.
    expect(runElapsed(view(['2026-09-16T10:00:00Z']))).toBeNull()
    expect(runElapsed(view([]))).toBeNull()
  })

  it('ignores a timestamp it cannot read rather than reporting a negative age', () => {
    const elapsed = runElapsed(view(['not-a-date', '2026-09-16T10:00:00Z', '2026-09-16T10:00:05Z']))

    expect(elapsed).toBe('5s')
  })
})

describe('answerElapsed', () => {
  const question = {
    occurredAt: '2026-09-16T10:00:00Z',
    type: 'message',
    role: 'user',
    final: true,
  }
  const answer = {
    occurredAt: '2026-09-16T10:33:20Z',
    type: 'message',
    role: 'assistant',
    final: true,
  }

  it('measures an answer from the entry before it, as the reference states it', () => {
    // A question at 10:00 answered at 10:33 took the model 33 minutes, and that
    // is the number the reference puts at the turn.
    expect(answerElapsed([question, answer], 1)).toBe('33m 20s')
  })

  it('states nothing for a row that is not a settled answer', () => {
    expect(answerElapsed([question, answer], 0)).toBeNull()
    expect(answerElapsed([question, { ...answer, final: false }], 1)).toBeNull()
    expect(answerElapsed([question, { ...answer, role: 'user' }], 1)).toBeNull()
    expect(answerElapsed([question, { ...answer, type: 'tool' }], 1)).toBeNull()
  })

  it('states nothing it could not measure', () => {
    // The first row has nothing before it, and an unreadable instant is not a
    // duration.
    expect(answerElapsed([answer], 0)).toBeNull()
    expect(answerElapsed([question, { ...answer, occurredAt: 'not-a-date' }], 1)).toBeNull()
    expect(answerElapsed([question, { ...answer, occurredAt: null }], 1)).toBeNull()
    // An answer that arrived in the same second as the question took no
    // measurable moment.
    expect(answerElapsed([question, { ...answer, occurredAt: question.occurredAt }], 1)).toBeNull()
  })
})