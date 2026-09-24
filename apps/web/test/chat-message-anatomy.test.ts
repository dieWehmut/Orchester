import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { createEmptyRunView, type MessageTimelineItem } from '@orchester/ereignis'
import type { SessionDetailDto } from '@orchester/protokoll'

import RunTimeline from '../src/components/run/RunTimeline.vue'
import RunFooter from '../src/components/run/RunFooter.vue'
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

  it('renders the markdown as it arrives, the way the reference does', () => {
    const wrapper = mountTimeline([message({ text: '```ts\nconst a =', final: false })])

    // The reference formats an answer while it is still being written, and this
    // parser reads an unclosed fence as the code it already is, so the block the
    // reader watches appear is the block they end up with.
    const row = wrapper.get('[data-item-type="message"]')

    expect(row.attributes('data-arrival-state')).toBe('streaming')
    expect(wrapper.get('[data-markdown-code]').text()).toContain('const a =')
    expect(wrapper.find('[data-message-plain]').exists()).toBe(false)
  })

  it('contains an arriving row so its growth cannot move the transcript', () => {
    const wrapper = mountTimeline([message({ text: 'still arriving', final: false })])
    const row = wrapper.get('[data-item-type="message"]')

    // Formatting during arrival is only affordable because the row's growth is
    // contained: without it every token would be measured by the transcript's
    // own layout, which is what the virtual window reads.
    expect(row.attributes('style')).toContain('contain: layout paint')
  })

  it('marks where the stream crossed midnight, once per day', () => {
    const wrapper = mountTimeline([
      message({ key: 'm-1', occurredAt: new Date(2026, 8, 16, 21, 53).toISOString() }),
      message({ key: 'm-2', occurredAt: new Date(2026, 8, 16, 23, 58).toISOString() }),
      message({ key: 'm-3', occurredAt: new Date(2026, 8, 17, 0, 4).toISOString() }),
    ])

    const rows = wrapper.findAll('[data-item-type="message"]')
    const marks = wrapper.findAll('[data-day-separator]')

    expect(marks).toHaveLength(2)
    // The mark belongs to the first row of the day, so it is drawn inside that
    // row rather than between rows: the list measures rows by their position.
    expect(rows[0]!.find('[data-day-separator]').exists()).toBe(true)
    expect(rows[1]!.find('[data-day-separator]').exists()).toBe(false)
    expect(rows[2]!.find('[data-day-separator]').exists()).toBe(true)
    expect(rows[2]!.get('[data-day-separator] time').attributes('datetime')).toContain('2026')
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

  it('states how long the answer took, at the answer, as the reference does', () => {
    const wrapper = mountTimeline([
      message({
        key: 'm-1',
        role: 'user',
        text: 'what is in this repository',
        occurredAt: '2026-09-16T10:00:00Z',
      }),
      message({ key: 'm-2', occurredAt: '2026-09-16T10:33:20Z' }),
      // A second answer, measured from the one before it.
      message({ key: 'm-3', occurredAt: '2026-09-16T10:34:20Z' }),
    ])

    const rows = wrapper.findAll('[data-item-type="message"]')

    // The question carries no clock - it did not take the model any time - and
    // the answer carries the interval the journal measured.
    expect(rows[0]!.find('[data-message-duration]').exists()).toBe(false)
    expect(rows[1]!.get('[data-message-duration]').text()).toBe('Took 33m 20s')
    expect(rows[2]!.get('[data-message-duration]').text()).toBe('Took 1m 0s')
  })

  it('leaves the clock off an answer that is still arriving', () => {
    const wrapper = mountTimeline([
      message({ key: 'm-1', role: 'user', occurredAt: '2026-09-16T10:00:00Z' }),
      message({ key: 'm-2', final: false, occurredAt: '2026-09-16T10:01:00Z' }),
    ])

    // A row still being written has not finished taking anything.
    expect(wrapper.find('[data-message-duration]').exists()).toBe(false)
  })

  it('keeps the ledger a ledger: the footer states no clock', () => {
    const view = {
      ...createEmptyRunView(),
      timeline: [
        message({ key: 'm-1', occurredAt: '2026-09-16T10:00:00Z' }),
        message({ key: 'm-2', occurredAt: '2026-09-16T10:33:20Z' }),
      ],
    }

    const wrapper = mount(RunFooter, { props: { view, sequenceLabel: 'Sequence' } })

    expect(wrapper.get('[data-run-footer]').text()).toContain('Sequence')
    // The answer states it, and the strip states it while a run is in flight; a
    // third copy under the transcript is a number in a place the reference does
    // not put it.
    expect(wrapper.find('[data-run-duration]').exists()).toBe(false)
  })
})