import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { createEmptyRunView, type MessageTimelineItem } from '@orchester/ereignis'
import type { SessionDetailDto } from '@orchester/protokoll'

import RunTimeline from '../src/components/run/RunTimeline.vue'
import SessionTranscript from '../src/components/sessions/SessionTranscript.vue'

/**
 * The conversation, as the reference draws it.
 *
 * A run's transcript is two things at once, and the reference only draws one of
 * them: the conversation - a question and an answer, read as prose - and the
 * governed record of what the agent did, which is cards and ledger numbers. So
 * messages and reasoning render in the conversation's idiom, while tool calls,
 * approvals and validations keep the run's.
 */

function message(overrides: Partial<MessageTimelineItem> = {}): MessageTimelineItem {
  return {
    type: 'message',
    key: 'message-1',
    sequence: 3,
    occurredAt: '2026-09-20T06:00:00Z',
    turnId: null,
    role: 'assistant',
    text: 'The runtime boundary is isolated.',
    final: true,
    ...overrides,
  }
}

function mountTimeline(items: ReturnType<typeof createEmptyRunView>['timeline']) {
  return mount(RunTimeline, { props: { view: { ...createEmptyRunView(), timeline: items } } })
}

const writes: string[] = []

beforeEach(() => {
  writes.length = 0
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: {
      writeText: (value: string) => {
        writes.push(value)
        return Promise.resolve()
      },
    },
  })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('transcript chat anatomy', () => {
  it('renders an answer as prose rather than as an event card', () => {
    const wrapper = mountTimeline([message()])

    const row = wrapper.get('[data-item-type="message"]')

    expect(row.attributes('data-item-role')).toBe('assistant')
    expect(row.attributes('data-message-shape')).toBe('prose')
    // The ledger number belongs to the run's record, not to the sentence the
    // agent answered with: the reference's answer carries no sequence at all.
    expect(row.find('[data-run-sequence]').exists()).toBe(false)
  })

  it("puts the reader's own turn in a bubble at the end of the measure", () => {
    const wrapper = mountTimeline([message({ role: 'user', text: 'Inspect the runtime.' })])

    const row = wrapper.get('[data-item-type="message"]')

    expect(row.attributes('data-item-role')).toBe('user')
    expect(row.attributes('data-message-shape')).toBe('bubble')
    expect(row.text()).toContain('Inspect the runtime.')
  })

  it("offers the answer's text as a copy, and says when it has it", async () => {
    vi.useFakeTimers()
    const wrapper = mountTimeline([message()])

    const button = wrapper.get('[data-message-copy]')
    expect(button.attributes('data-copied')).toBe('false')

    await button.trigger('click')

    expect(writes).toEqual(['The runtime boundary is isolated.'])
    expect(button.attributes('data-copied')).toBe('true')
    expect(wrapper.get('[data-message-copy-status]').text().length).toBeGreaterThan(0)

    // The confirmation is a moment, not a state: the control returns to being
    // the control that copies.
    vi.advanceTimersByTime(3000)
    await wrapper.vm.$nextTick()

    expect(button.attributes('data-copied')).toBe('false')
  })

  it("keeps the ledger on the run's own artifacts", () => {
    const wrapper = mountTimeline([
      {
        type: 'tool',
        key: 'tool-1',
        sequence: 4,
        occurredAt: '2026-09-20T06:00:01Z',
        turnId: null,
        callId: 'call-1' as never,
        name: 'read_file',
        state: 'succeeded',
        detail: null,
      },
    ])

    const row = wrapper.get('[data-item-type="tool"]')

    expect(row.get('[data-run-sequence]').text()).toBe('4')
    expect(row.get('[data-tool-card]')).toBeTruthy()
  })

  it('renders the answer as the markdown it settled into', () => {
    const wrapper = mountTimeline([
      message({ text: 'Fixed it.\n\n```ts\nconst a = 1\n```\n\nSee [the PR](https://example.com/pr/1).' }),
    ])

    // The reference draws an answer's code, lists and links as themselves; a
    // coding agent's answer is written in markdown and reads as raw text
    // without this.
    expect(wrapper.get('[data-markdown-code]').text()).toContain('const a = 1')
    expect(wrapper.get('[data-markdown-link]').attributes('rel')).toBe('noopener noreferrer')
  })

  it('keeps the text literal while it is still arriving', () => {
    const wrapper = mountTimeline([message({ text: '```ts\nconst a =', final: false })])

    // A fence that is half written is not a code block yet: formatting arrives
    // with the text, so the paragraph cannot flicker apart under the reader.
    expect(wrapper.find('[data-markdown-code]').exists()).toBe(false)
    expect(wrapper.get('[data-message-plain]').text()).toContain('const a =')
  })

  it("shows the reader's own words exactly as they typed them", () => {
    const wrapper = mountTimeline([
      message({ role: 'user', text: '**not bold** and `literal`' }),
    ])

    const body = wrapper.get('[data-message-plain]')

    expect(body.text()).toBe('**not bold** and `literal`')
    expect(wrapper.find('[data-markdown-inline-code]').exists()).toBe(false)
  })
})

const detail: SessionDetailDto = {
  id: 's-11111111111111111111111111111111',
  source: 'delegate',
  recorded_at_unix: 1_700_000_000,
  title: 'Inspect the runtime',
  agent: 'codex',
  model: 'gpt-5',
  outcome: 'success',
  resumable: true,
  schema_version: 1,
  prompt: 'Inspect the runtime boundaries.',
  final_text: 'The runtime boundary is isolated.',
  usage: {
    input_tokens: 20,
    output_tokens: 10,
    cached_input_tokens: 5,
    reasoning_output_tokens: 2,
  },
}

describe('stored session anatomy', () => {
  it('reads a stored session as the conversation it was', () => {
    const wrapper = mount(SessionTranscript, {
      props: { status: 'ready', session: detail, error: null },
    })

    const question = wrapper.get('[data-message-role="user"]')
    const answer = wrapper.get('[data-message-role="assistant"]')

    expect(question.attributes('data-message-shape')).toBe('bubble')
    expect(question.text()).toContain(detail.prompt)
    expect(answer.attributes('data-message-shape')).toBe('prose')
    expect(answer.text()).toContain(detail.final_text)

    // The role is carried by the shape, as the reference carries it, rather
    // than by an uppercase heading over each half.
    expect(wrapper.findAll('h2')).toHaveLength(0)
  })

  it('offers the stored answer as a copy too', async () => {
    const wrapper = mount(SessionTranscript, {
      props: { status: 'ready', session: detail, error: null },
    })

    await wrapper.get('[data-message-role="assistant"] [data-message-copy]').trigger('click')

    expect(writes).toEqual([detail.final_text])
  })
})