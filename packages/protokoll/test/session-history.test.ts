import { describe, expect, it } from 'vitest'

import {
  SESSION_HISTORY_SCHEMA_VERSION,
  type SessionDetailDto,
  type SessionPageDto,
} from '../src/index'

describe('session history DTOs', () => {
  it('keeps summaries bounded and paginated without native session IDs', () => {
    const page: SessionPageDto = {
      schema_version: SESSION_HISTORY_SCHEMA_VERSION,
      items: [
        {
          id: 's-0123456789abcdef0123456789abcdef',
          source: 'delegate',
          recorded_at_unix: 1_800_000_000,
          title: 'Review the workspace changes',
          agent: 'codex',
          model: 'gpt-5.6',
          outcome: 'success',
          resumable: true,
        },
      ],
      next_cursor: 's-0123456789abcdef0123456789abcdef',
    }

    expect(page.items[0]?.id).toMatch(/^s-[0-9a-f]{32}$/)
    expect(page.items[0]).not.toHaveProperty('prompt')
    expect(page.items[0]).not.toHaveProperty('native_session_id')
    expect(page.items[0]).not.toHaveProperty('cwd')
  })

  it('adds transcript text and usage only on the detail response', () => {
    const detail: SessionDetailDto = {
      schema_version: SESSION_HISTORY_SCHEMA_VERSION,
      id: 's-0123456789abcdef0123456789abcdef',
      source: 'delegate',
      recorded_at_unix: 1_800_000_000,
      title: 'Review the workspace changes',
      agent: 'codex',
      model: 'gpt-5.6',
      outcome: 'success',
      resumable: true,
      prompt: 'Review the workspace changes',
      final_text: 'The review is complete.',
      usage: {
        input_tokens: 12,
        output_tokens: 8,
        cached_input_tokens: 0,
        reasoning_output_tokens: 2,
      },
    }

    expect(detail.final_text).toContain('complete')
    expect(JSON.stringify(detail)).not.toContain('C:\\')
  })
})
