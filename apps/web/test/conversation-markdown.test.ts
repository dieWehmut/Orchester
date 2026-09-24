import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'

import { conversationMarkdown } from '../src/components/run/conversation-markdown'

/**
 * What leaves when a reader copies the conversation.
 *
 * The reference's share publishes the thread and hands back a link; there is no
 * host here to publish to, so what leaves is text - and the shape of that text
 * is what makes a paste read as a conversation rather than as two blobs.
 */

function message(
  key: string,
  role: 'user' | 'assistant',
  text: string,
  occurredAt = '2026-09-20T06:00:00Z',
) {
  return {
    type: 'message' as const,
    key,
    sequence: 1,
    turnId: null,
    occurredAt,
    role,
    text,
    final: true,
  }
}

describe('conversationMarkdown', () => {
  it('writes the title, the reader’s words as a quotation and the answer as prose', () => {
    const markdown = conversationMarkdown({
      ...createEmptyRunView(),
      title: 'Inspect the runtime',
      timeline: [
        message('m-1', 'user', 'Inspect the runtime.\nAnd the boundaries.'),
        message('m-2', 'assistant', 'The boundary is isolated.'),
      ],
    })

    expect(markdown).toBe(
      [
        '# Inspect the runtime',
        '',
        '> Inspect the runtime.',
        '> And the boundaries.',
        '',
        'The boundary is isolated.',
      ].join('\n'),
    )
  })

  it('keeps an empty line of a question a quotation too', () => {
    const markdown = conversationMarkdown({
      ...createEmptyRunView(),
      timeline: [message('m-1', 'user', 'first\n\nsecond')],
    })

    // `>` alone is a quotation of nothing, which is what the reader wrote.
    expect(markdown).toBe('> first\n>\n> second')
  })

  it('leaves the run’s own record out: a shared thread is what was said', () => {
    const markdown = conversationMarkdown({
      ...createEmptyRunView(),
      timeline: [
        message('m-1', 'user', 'question'),
        {
          type: 'tool',
          key: 't-1',
          sequence: 2,
          turnId: null,
          occurredAt: '2026-09-20T06:00:02Z',
          callId: 'call-1' as never,
          name: 'read_file',
          state: 'succeeded',
          detail: 'src/lib.rs',
        },
        message('m-2', 'assistant', 'answer'),
        {
          type: 'reasoning',
          key: 'r-1',
          sequence: 4,
          turnId: null,
          occurredAt: '2026-09-20T06:00:03Z',
          text: 'thinking about it',
        },
      ],
    })

    expect(markdown).toBe('> question\n\nanswer')
  })

  it('has nothing to copy when nothing has been said', () => {
    expect(conversationMarkdown(createEmptyRunView())).toBe('')
    // A run with no title is still a conversation.
    const untitled = conversationMarkdown({
      ...createEmptyRunView(),
      timeline: [message('m-1', 'assistant', 'just an answer')],
    })
    expect(untitled).toBe('just an answer')
  })
})