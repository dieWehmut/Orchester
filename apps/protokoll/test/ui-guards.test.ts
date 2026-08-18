import { describe, expect, it } from 'vitest'

import { parseUiEventEnvelope, parseUiEventEnvelopeJson } from '../src/index'

function validEnvelope(): Record<string, unknown> {
  return {
    schema_version: 1,
    event_id: 'event-1',
    run_id: 'run-1',
    turn_id: 'turn-1',
    call_id: 'call-1',
    sequence: 1,
    occurred_at: '2026-08-19T00:00:00Z',
    kind: {
      type: 'tool_call',
      call_id: 'call-1',
      name: 'read_file',
      state: 'running',
      detail: 'src/main.rs',
    },
  }
}

describe('parseUiEventEnvelope', () => {
  it('parses a valid envelope and preserves branded ids as strings', () => {
    expect(parseUiEventEnvelope(validEnvelope())).toEqual(validEnvelope())
  })

  it('rejects unsupported schemas and invalid sequence numbers', () => {
    expect(parseUiEventEnvelope({ ...validEnvelope(), schema_version: 2 })).toBeNull()
    expect(parseUiEventEnvelope({ ...validEnvelope(), sequence: 0 })).toBeNull()
    expect(parseUiEventEnvelope({ ...validEnvelope(), sequence: 1.5 })).toBeNull()
  })

  it('rejects empty correlation identifiers and timestamps', () => {
    expect(parseUiEventEnvelope({ ...validEnvelope(), event_id: ' ' })).toBeNull()
    expect(parseUiEventEnvelope({ ...validEnvelope(), run_id: '' })).toBeNull()
    expect(parseUiEventEnvelope({ ...validEnvelope(), occurred_at: ' ' })).toBeNull()
  })

  it('requires the tool payload call id to match the envelope', () => {
    expect(parseUiEventEnvelope({ ...validEnvelope(), call_id: 'call-2' })).toBeNull()
    const { call_id: _callId, ...withoutCallId } = validEnvelope()
    expect(parseUiEventEnvelope(withoutCallId)).toBeNull()
  })

  it('binds approval requests to the run and a positive row version', () => {
    const { call_id: _callId, ...approvalBase } = validEnvelope()
    const approval = {
      ...approvalBase,
      kind: {
        type: 'approval_requested',
        approval: {
          approval_id: 'approval-1',
          run_id: 'run-1',
          row_version: 1,
          risk: 'high',
          action: 'write_file path=src/main.rs',
          reason: 'workspace write',
        },
      },
    }
    expect(parseUiEventEnvelope(approval)).not.toBeNull()
    expect(
      parseUiEventEnvelope({
        ...approval,
        kind: {
          ...approval.kind,
          approval: { ...approval.kind.approval, run_id: 'run-2' },
        },
      }),
    ).toBeNull()
    expect(
      parseUiEventEnvelope({
        ...approval,
        kind: {
          ...approval.kind,
          approval: { ...approval.kind.approval, row_version: 0 },
        },
      }),
    ).toBeNull()
  })

  it('returns null for malformed JSON', () => {
    expect(parseUiEventEnvelopeJson('{"schema_version":')).toBeNull()
  })

  it('rejects duplicate envelope and nested fields before values are overwritten', () => {
    const duplicateSequence = JSON.stringify(validEnvelope()).replace(
      '"sequence":1',
      '"sequence":1,"sequence":2',
    )
    const duplicateState = JSON.stringify(validEnvelope()).replace(
      '"state":"running"',
      '"state":"running","state":"succeeded"',
    )

    expect(parseUiEventEnvelopeJson(duplicateSequence)).toBeNull()
    expect(parseUiEventEnvelopeJson(duplicateState)).toBeNull()
  })

  it('rejects every missing required envelope field', () => {
    for (const key of [
      'schema_version',
      'event_id',
      'run_id',
      'sequence',
      'occurred_at',
      'kind',
    ] as const) {
      const candidate = validEnvelope()
      delete candidate[key]
      expect(parseUiEventEnvelope(candidate), key).toBeNull()
    }
  })

  it('rejects unknown outer, kind, and nested fields', () => {
    expect(parseUiEventEnvelope({ ...validEnvelope(), provider_payload: {} })).toBeNull()
    expect(
      parseUiEventEnvelope({
        ...validEnvelope(),
        kind: { ...(validEnvelope().kind as Record<string, unknown>), raw_arguments: [] },
      }),
    ).toBeNull()

    const { call_id: _callId, ...approvalBase } = validEnvelope()
    expect(
      parseUiEventEnvelope({
        ...approvalBase,
        kind: {
          type: 'approval_requested',
          approval: {
            approval_id: 'approval-1',
            run_id: 'run-1',
            row_version: 1,
            risk: 'high',
            action: 'write_file path=src/main.rs',
            reason: 'workspace write',
            action_hash: 'must-not-cross-the-browser-boundary',
          },
        },
      }),
    ).toBeNull()
  })
})
