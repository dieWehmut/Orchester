import { describe, expect, it } from 'vitest'

import { petStateFor, type PetStateInput } from '../src/features/pet/pet-state'

function input(overrides: Partial<PetStateInput> = {}): PetStateInput {
  return {
    runStatus: 'idle',
    busy: false,
    pendingApprovals: 0,
    errorMessage: null,
    ...overrides,
  }
}

describe('pet companion state', () => {
  it('rests on idle when nothing is happening', () => {
    expect(petStateFor(input())).toEqual({ animation: 'idle', notification: null })
  })

  it('mirrors an active run', () => {
    expect(petStateFor(input({ runStatus: 'running' }))).toEqual({
      animation: 'running',
      notification: 'running',
    })
    expect(petStateFor(input({ busy: true }))).toEqual({
      animation: 'running',
      notification: 'running',
    })
  })

  it('asks for input while an approval is pending', () => {
    expect(petStateFor(input({ pendingApprovals: 1, runStatus: 'running' }))).toEqual({
      animation: 'waiting',
      notification: 'waiting',
    })
    expect(petStateFor(input({ runStatus: 'awaiting_approval' }))).toEqual({
      animation: 'waiting',
      notification: 'waiting',
    })
  })

  it('shows a failed pose for a failure or a surfaced error', () => {
    expect(petStateFor(input({ runStatus: 'failed' }))).toEqual({
      animation: 'failed',
      notification: 'failed',
    })
    expect(petStateFor(input({ errorMessage: 'Runtime unreachable' }))).toEqual({
      animation: 'failed',
      notification: 'failed',
    })
    expect(petStateFor(input({ runStatus: 'interrupted_unknown_outcome' })).notification).toBe(
      'failed',
    )
  })

  it('holds a review pose after the run ends', () => {
    expect(petStateFor(input({ runStatus: 'succeeded' }))).toEqual({
      animation: 'review',
      notification: 'review',
    })
    expect(petStateFor(input({ runStatus: 'cancelled' })).animation).toBe('review')
  })

  it('prefers waiting over a stale failure, and failure over a stale run', () => {
    expect(
      petStateFor(input({ runStatus: 'failed', pendingApprovals: 1 })).animation,
    ).toBe('waiting')
    expect(petStateFor(input({ runStatus: 'failed', busy: true })).animation).toBe('failed')
  })
})
