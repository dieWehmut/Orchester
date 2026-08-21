import { describe, expect, it } from 'vitest'

import { failurePathFixture, parseUiEventEnvelope } from '../../src/index'

describe('failure path fixture', () => {
  it('records failed tool and validation before a failed terminal state', () => {
    const events = failurePathFixture()
    const toolFailure = events.find(
      (event) => event.kind.type === 'tool_call' && event.kind.state === 'failed',
    )
    const validationFailure = events.find(
      (event) => event.kind.type === 'validation' && !event.kind.validation.ok,
    )
    const terminal = events.at(-1)

    expect(toolFailure).toBeDefined()
    expect(validationFailure).toBeDefined()
    expect(terminal?.kind).toEqual({ type: 'run_stopped', reason: 'failed' })
    expect(events.every((event) => parseUiEventEnvelope(event) !== null)).toBe(true)
  })
})
