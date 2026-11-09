import { describe, expect, it } from 'vitest'
import type { SessionDetailDto, UiEventEnvelope } from '@orchester/protokoll'
import { conversationMarkdown, conversationUrl } from '../src/components/sessions/session-export'

const detail = { id: 's-11111111111111111111111111111111', title: 'Saved thread', prompt: 'First question', final_text: 'Answer' } as SessionDetailDto
function event(run: string, sequence: number, kind: UiEventEnvelope['kind']): UiEventEnvelope {
  return { schema_version: 1, event_id: `${run}-${sequence}`, run_id: run, sequence, occurred_at: '2026-11-05T01:00:00Z', turn_id: `${run}-turn`, kind } as UiEventEnvelope
}
describe('conversation export', () => {
  it('preserves code fences and all questions across separate runs', () => {
    const saved = { ...detail, events: [
      event('r1', 1, { type: 'user_message', text: 'Question 1' }),
      event('r1', 2, { type: 'message', text: '```ts\nconst value = 1\n```' }),
      event('r2', 1, { type: 'user_message', text: 'Question 2' }),
      event('r2', 2, { type: 'message', text: '| A | B |\n| --- | --- |\n| 1 | 2 |' }),
    ] }
    const result = conversationMarkdown(saved, 'Renamed thread')
    expect(result).toContain('# Renamed thread')
    expect(result).toContain('## User\n\nQuestion 1')
    expect(result).toContain('## User\n\nQuestion 2')
    expect(result).toContain('```ts\nconst value = 1\n```')
    expect(result).toContain('| 1 | 2 |')
  })
  it('exports delegated histories without a journal', () => {
    expect(conversationMarkdown(detail)).toBe('# Saved thread\n\n## User\n\nFirst question\n\n## Assistant\n\nAnswer\n')
  })
  it('never includes bootstrap fragments or unrelated query parameters in shared links', () => {
    const url = new URL(conversationUrl(detail.id, 'http://127.0.0.1:3000/settings?other=secret#token=secret'))
    expect(url.pathname).toBe('/workspace')
    expect(url.searchParams.get('session')).toBe(detail.id)
    expect([...url.searchParams.keys()]).toEqual(['session'])
    expect(url.hash).toBe('')
  })
})
