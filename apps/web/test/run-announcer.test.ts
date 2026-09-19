import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'

import { fixtureEnvelope, type UiEventEnvelope } from '@orchester/protokoll'

import RunAnnouncer from '../src/components/run/RunAnnouncer.vue'

/**
 * The live regions the panel actually ships.
 *
 * The pure mapping is pinned in `run-announcements.test.ts`; this pins the two
 * things only the rendered component can get wrong: which region an
 * announcement lands in, and whether the region exists before it has anything
 * to say. A region created along with its first message is one assistive
 * technology was never watching.
 */

function mountAnnouncer(events: readonly UiEventEnvelope[] = []) {
  return mount(RunAnnouncer, { props: { events: [...events] } })
}

describe('RunAnnouncer', () => {
  it('ships both regions empty, in the document, before anything happens', () => {
    const wrapper = mountAnnouncer()

    const polite = wrapper.get('[data-run-announcer="polite"]')
    const assertive = wrapper.get('[data-run-announcer="assertive"]')

    expect(polite.attributes('aria-live')).toBe('polite')
    expect(polite.attributes('role')).toBe('status')
    expect(assertive.attributes('aria-live')).toBe('assertive')
    expect(assertive.attributes('role')).toBe('alert')
    expect(polite.text()).toBe('')
    expect(assertive.text()).toBe('')
  })

  it('reads the journal as news but leaves the mounted history unread', async () => {
    const history = [
      fixtureEnvelope(1, { type: 'run_started', title: 'Inspect' }),
      fixtureEnvelope(2, { type: 'message', text: 'Old reply' }),
    ]
    const wrapper = mountAnnouncer(history)

    // The history was already on screen when the panel mounted; narrating it
    // would make the reader listen to the transcript again.
    expect(wrapper.get('[data-run-announcer="polite"]').text()).toBe('')

    await wrapper.setProps({
      events: [...history, fixtureEnvelope(3, { type: 'validation', validation: { ok: true, summary: 'All checks passed' } })],
    })
    await nextTick()

    expect(wrapper.get('[data-run-announcer="polite"]').text()).toContain('All checks passed')
  })

  it('routes a failure to the assertive region and a result to the polite one', async () => {
    const events: UiEventEnvelope[] = [fixtureEnvelope(1, { type: 'run_started' })]
    const wrapper = mountAnnouncer(events)

    await wrapper.setProps({
      events: [...events, fixtureEnvelope(2, { type: 'message_delta', text: 'Done', final: true })],
    })
    await nextTick()
    expect(wrapper.get('[data-run-announcer="polite"]').text()).toContain('Turn complete')

    await wrapper.setProps({
      events: [
        ...events,
        fixtureEnvelope(2, { type: 'message_delta', text: 'Done', final: true }),
        fixtureEnvelope(3, { type: 'run_stopped', reason: 'failed' }),
      ],
    })
    await nextTick()
    expect(wrapper.get('[data-run-announcer="assertive"]').text()).toContain('Run failed')
  })

  it('never narrates the streamed chunks of a reply', async () => {
    const events: UiEventEnvelope[] = [fixtureEnvelope(1, { type: 'run_started' })]
    const wrapper = mountAnnouncer(events)

    const grown = [...events]
    for (const [index, chunk] of ['a', 'b', 'c'].entries()) {
      grown.push(fixtureEnvelope(index + 2, { type: 'message_delta', text: chunk, final: false }))
      await wrapper.setProps({ events: [...grown] })
      await nextTick()
    }

    // The run started before the panel was listening, and no chunk is news, so
    // the region is still empty: text arriving is not something to read out.
    expect(wrapper.get('[data-run-announcer="polite"]').text()).toBe('')
  })
})
