import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'
import RunPanel from '../src/components/run/RunPanel.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('RunPanel', () => {
  it('renders an actionable empty run with composer and greeting, and no ledger yet', () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })

    expect(wrapper.get('[data-run-panel]')).toBeTruthy()
    expect(wrapper.get('[data-run-composer]')).toBeTruthy()
    expect(wrapper.get('[data-empty-workspace]')).toBeTruthy()
    expect(wrapper.get('[data-orchester-mark]')).toBeTruthy()
    // A ledger under an empty page reports a run that has not happened; the
    // reference's home is the greeting and the field and nothing else. The
    // ledger appears with the first event - see the hero test below.
    expect(wrapper.find('[data-run-footer]').exists()).toBe(false)
  })

  it('removes the large mark immediately after a conversation starts', async () => {
    const wrapper = mount(RunPanel, {
      props: { view: createEmptyRunView(), conversationStarted: false },
    })

    expect(wrapper.find('[data-orchester-mark]').exists()).toBe(true)
    await wrapper.setProps({ conversationStarted: true, busy: true })

    expect(wrapper.find('[data-orchester-mark]').exists()).toBe(false)
    expect(wrapper.get('[data-run-awaiting-events]')).toBeTruthy()
    expect(wrapper.get('[data-run-composer]')).toBeTruthy()
  })

  it('forwards submit and cancel intents without fetching', async () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })
    const textarea = wrapper.get('textarea')

    await textarea.setValue('Inspect the workspace')
    await textarea.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('submit')).toEqual([['Inspect the workspace']])

    await wrapper.setProps({ busy: true })
    await wrapper.get('[data-composer-action="cancel"]').trigger('click')
    expect(wrapper.emitted('cancel')).toHaveLength(1)
  })

  it('forwards workspace and model state into the composer context', () => {
    const wrapper = mount(RunPanel, {
      props: {
        view: createEmptyRunView(),
        workspaceName: 'Orchester',
        modelCatalog: MODEL_CATALOG_FIXTURE,
        modelStatus: 'ready',
      },
    })

    expect(wrapper.get('[data-project-context]').text()).toContain('Orchester')
    expect(wrapper.get('[data-model-context-model]').text()).toContain('gpt-5.6')
  })

  it('draws the ambient companion and mirrors the run state', async () => {
    const wrapper = mount(RunPanel, {
      props: {
        view: createEmptyRunView(),
        // A page with a run on it: the companion reacts to run state, and the
        // home page before the first run has none to react to.
        conversationStarted: true,
        runStatus: 'running',
        petLabel: 'Orchester companion',
      },
    })

    const companion = wrapper.get('[data-run-companion] [data-pet-companion]')
    expect(companion.attributes('data-pet-animation')).toBe('running')
    expect(companion.attributes('aria-label')).toBe('Orchester companion')
  })

  it('hands the companion a notification label while a decision is pending', async () => {
    const wrapper = mount(RunPanel, {
      props: {
        view: createEmptyRunView(),
        conversationStarted: true,
        pendingApprovals: 1,
        petNotificationLabels: { waiting: 'Needs input' },
      },
    })

    expect(wrapper.get('[data-run-companion] [data-pet-companion]').attributes('data-pet-animation')).toBe(
      'waiting',
    )
  })

  it('animates a run activity indicator only while a run is busy', async () => {
    const wrapper = mount(RunPanel, {
      props: { view: createEmptyRunView(), busy: true },
    })

    const indicator = wrapper.get('[data-run-activity]')
    expect(indicator.attributes('role')).toBe('status')
    expect(indicator.attributes('aria-label')).toBe('Run in progress')

    await wrapper.setProps({ busy: false })
    expect(wrapper.find('[data-run-activity]').exists()).toBe(false)
  })

  it('states the run above the field while it is in flight, and stops when it settles', () => {
    // The reference draws this strip directly above the input: the state, the
    // title the runtime gave the run, and a live clock at the far end.
    const running = {
      ...createEmptyRunView(),
      status: 'running' as const,
      title: '优化 VerifierLab 可视化 UI',
      timeline: [
        {
          key: 'm-1',
          sequence: 1,
          turnId: null,
          occurredAt: '2026-09-16T10:00:00Z',
          type: 'message' as const,
          role: 'user' as const,
          text: '优化 VerifierLab 可视化 UI',
          final: true,
        },
        {
          key: 'm-2',
          sequence: 2,
          turnId: null,
          occurredAt: '2026-09-16T10:03:20Z',
          type: 'message' as const,
          role: 'assistant' as const,
          text: 'working',
          final: true,
        },
      ],
    }

    const wrapper = mount(RunPanel, { props: { view: running } })
    const strip = wrapper.get('[data-run-state]')

    expect(strip.attributes('data-run-state-status')).toBe('running')
    expect(strip.get('[data-run-state-label]').text()).toBe('Running')
    expect(strip.get('[data-run-state-title]').text()).toBe('优化 VerifierLab 可视化 UI')
    expect(strip.get('[data-run-state-elapsed]').text()).toBe('Took 3m 20s')
    // The footer is the ledger, not a second clock: the strip counts while a run
    // is in flight and each answer states what it took.
    expect(wrapper.find('[data-run-duration]').exists()).toBe(false)

    // Once it has settled the strip goes, and the answer carries the interval.
    const settled = mount(RunPanel, {
      props: { view: { ...running, status: 'succeeded' as const } },
    })
    expect(settled.find('[data-run-state]').exists()).toBe(false)
    expect(settled.get('[data-message-duration]').text()).toBe('Took 3m 20s')
  })

  it('names a run waiting on a human as waiting, not as working', () => {
    const wrapper = mount(RunPanel, {
      props: {
        view: { ...createEmptyRunView(), status: 'awaiting_approval' as const },
      },
    })

    const strip = wrapper.get('[data-run-state]')
    expect(strip.attributes('data-run-state-status')).toBe('awaiting_approval')
    expect(strip.get('[data-run-state-label]').text()).toBe('Waiting for approval')
    // No clock yet: the run has taken no measurable moment in this fixture.
    expect(strip.find('[data-run-state-elapsed]').exists()).toBe(false)
  })

  it('keeps the companion off the page that has no run to react to', async () => {
    const { resetPetVisibilityForTests, usePetVisibility } = await import(
      '../src/features/pet/use-pet-visibility'
    )
    resetPetVisibilityForTests()
    const pet = usePetVisibility()
    pet.show()

    // The companion has an animation per run state; on the page before the
    // first run it would also sit between the greeting and the field, which the
    // reference draws as one group.
    const empty = mount(RunPanel, { props: { view: createEmptyRunView() } })
    expect(empty.find('[data-run-companion]').exists()).toBe(false)

    const started = mount(RunPanel, {
      props: { view: createEmptyRunView(), conversationStarted: true },
    })
    expect(started.find('[data-run-companion]').exists()).toBe(true)
    resetPetVisibilityForTests()
  })

  it('greets a new page with the field under the greeting, and docks it once there is a transcript', () => {
    // The reference greets a new chat with its greeting and its field as one
    // group in the middle, and pins the field to the bottom only once there is
    // something to scroll.
    const empty = mount(RunPanel, { props: { view: createEmptyRunView() } })

    expect(empty.get('[data-run-panel]').attributes('data-run-hero')).toBe('true')
    expect(empty.find('[data-empty-workspace]').exists()).toBe(true)
    // Nothing to report yet, so the run's ledger is not drawn under the greeting
    // where it would read as a fact about this page.
    expect(empty.find('[data-run-footer]').exists()).toBe(false)
    expect(empty.find('[data-composer-field]').exists()).toBe(true)

    const started = mount(RunPanel, {
      props: {
        view: createEmptyRunView(),
        conversationStarted: true,
      },
    })
    expect(started.get('[data-run-panel]').attributes('data-run-hero')).toBe('false')
    expect(started.find('[data-run-footer]').exists()).toBe(true)

    // A transcript is a conversation too, whatever the flag says.
    const withTimeline = mount(RunPanel, {
      props: {
        view: {
          ...createEmptyRunView(),
          timeline: [
            {
              key: 'm-1',
              sequence: 1,
              turnId: null,
              occurredAt: '2026-09-16T10:00:00Z',
              type: 'message' as const,
              role: 'user' as const,
              text: 'hello',
              final: true,
            },
          ],
        },
      },
    })
    expect(withTimeline.get('[data-run-panel]').attributes('data-run-hero')).toBe('false')
  })

  it('names the composer state from the run lifecycle the panel is given', async () => {
    const wrapper = mount(RunPanel, {
      props: { view: createEmptyRunView(), lifecycle: 'submitting' },
    })

    expect(wrapper.get('[data-run-composer]').attributes('data-composer-state')).toBe('submitting')

    await wrapper.setProps({ lifecycle: 'running' })
    expect(wrapper.get('[data-run-composer]').attributes('data-composer-state')).toBe('running')
  })

  it('shows the plan strip above the composer once the run has a plan', () => {
    const view = createEmptyRunView()
    const withPlan = {
      ...view,
      todos: [
        { text: 'Read the runtime', completed: true },
        { text: 'Patch the boundary', completed: false },
      ],
    }
    const wrapper = mount(RunPanel, { props: { view: withPlan } })

    const strip = wrapper.get('[data-plan-strip]')
    expect(strip.attributes('data-plan-state')).toBe('active')
    expect(strip.get('[data-plan-current]').text()).toContain('Patch the boundary')

    // The strip sits between the transcript and the composer: it describes the
    // run, so it belongs above the input that continues it.
    const stream = wrapper.get('[data-run-panel] .run-panel__stream').element
    const composer = wrapper.get('[data-run-composer]').element
    expect(
      strip.element.compareDocumentPosition(stream) & Node.DOCUMENT_POSITION_PRECEDING,
    ).toBeTruthy()
    expect(
      strip.element.compareDocumentPosition(composer) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('hides the plan strip when the run has no plan', () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })

    expect(wrapper.find('[data-plan-strip]').exists()).toBe(false)
  })

  it('marks the plan blocked while the run is waiting for the user', () => {
    const view = createEmptyRunView()
    const wrapper = mount(RunPanel, {
      props: {
        view: {
          ...view,
          status: 'awaiting_approval',
          todos: [{ text: 'Wait for the approval', completed: false }],
        },
      },
    })

    const strip = wrapper.get('[data-plan-strip]')
    expect(strip.attributes('data-plan-state')).toBe('blocked')
    expect(strip.attributes('data-plan-needs-input')).toBe('true')
  })

  it('reports the transcript scroll flags and hides the scroll-to-bottom control', () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })
    const stream = wrapper.get('[data-transcript-scroll]')

    // jsdom reports a zero-height box; the reader starts where content that
    // fits leaves them: at the bottom, with nothing to scroll to.
    expect(stream.attributes('data-can-scroll-up')).toBe('false')
    expect(stream.attributes('data-can-scroll-down')).toBe('false')
    expect(wrapper.find('[data-scroll-to-bottom]').exists()).toBe(false)
  })

  it('announces output that arrives while the reader is reading back', async () => {
    const view = createEmptyRunView()
    const wrapper = mount(RunPanel, { props: { view } })
    const stream = wrapper.get('[data-transcript-scroll]').element as HTMLElement

    // Scroll the reader away from the bottom, then let a turn arrive. The jump
    // writes scrollTop, so the stub has to accept the write.
    Object.defineProperty(stream, 'scrollTop', { value: 100, writable: true, configurable: true })
    Object.defineProperty(stream, 'scrollHeight', { value: 1200, writable: true, configurable: true })
    Object.defineProperty(stream, 'clientHeight', { value: 400, writable: true, configurable: true })
    await stream.dispatchEvent(new Event('scroll'))

    await wrapper.setProps({
      view: {
        ...view,
        timeline: [
          {
            type: 'message' as const,
            key: 'message-1',
            sequence: 1,
            occurredAt: '2026-09-20T06:00:00Z',
            turnId: null,
            role: 'assistant' as const,
            text: 'still working',
            final: true,
          },
        ],
      },
    })
    // The watcher defers a tick before deciding, so it measures the transcript
    // the new turn actually produced.
    await flushPromises()

    // The run kept producing while they read, so the control says how much
    // rather than pretending nothing happened.
    expect(wrapper.get('[data-scroll-unread-dot]').text()).toContain('1')
    expect(wrapper.get('[data-scroll-to-bottom]').attributes('data-scroll-unread')).toBe('true')
    // The reference draws a round control that points down, with the news riding
    // its corner; the count is what the control says out loud, not its face.
    expect(wrapper.get('[data-scroll-to-bottom]').attributes('data-scroll-shape')).toBe('jump')
    expect(wrapper.get('[data-scroll-to-bottom]').attributes('aria-label')).toContain('1')

    // Asking for the bottom is also how the reader marks it read; the count may
    // not survive the jump or the control would nag forever. Landing at the
    // bottom also retires the control itself, because there is nowhere to jump.
    await wrapper.get('[data-scroll-to-bottom]').trigger('click')
    expect(wrapper.find('[data-scroll-unread-dot]').exists()).toBe(false)
    expect(wrapper.find('[data-scroll-to-bottom]').exists()).toBe(false)
  })

  it('mounts the message rail on the transcript and jumps to the turn chosen', async () => {
    const view = {
      ...createEmptyRunView(),
      timeline: [
        {
          type: 'message' as const,
          key: 'message-1',
          sequence: 1,
          occurredAt: '2026-09-20T06:00:00Z',
          turnId: null,
          role: 'user' as const,
          text: 'first question',
          final: true,
        },
        {
          type: 'message' as const,
          key: 'message-2',
          sequence: 2,
          occurredAt: '2026-09-20T06:01:00Z',
          turnId: null,
          role: 'user' as const,
          text: 'second question',
          final: true,
        },
      ],
    }
    const wrapper = mount(RunPanel, { props: { view } })

    const rail = wrapper.get('[data-message-rail]')
    expect(rail.findAll('[data-rail-mark]')).toHaveLength(2)

    // jsdom has no layout, so scrollIntoView is recorded rather than performed.
    const scrolled: number[] = []
    for (const row of wrapper.findAll('[data-virtualized-turn]')) {
      Object.defineProperty(row.element, 'scrollIntoView', {
        value: () => scrolled.push(Number(row.attributes('data-virtualized-turn'))),
      })
    }
    await rail.findAll('[data-rail-mark]')[1]!.trigger('click')
    expect(scrolled).toEqual([1])
  })

  it('carries the top fade as state rather than as an always-on decoration', async () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })
    const stream = wrapper.get('[data-transcript-scroll]').element as HTMLElement
    const fade = wrapper.get('[data-transcript-fade]')

    // Nothing is above the reader on a transcript that fits, so the fade is
    // hidden; the same element becomes visible once there is content above.
    expect(fade.attributes('data-transcript-fade')).toBe('hidden')

    // jsdom does not lay out, so the box the measurement reads is written by
    // hand: the same element the reader scrolls, now scrolled away from its top.
    Object.defineProperty(stream, 'scrollTop', { value: 200, configurable: true })
    Object.defineProperty(stream, 'scrollHeight', { value: 1200, configurable: true })
    Object.defineProperty(stream, 'clientHeight', { value: 400, configurable: true })
    await stream.dispatchEvent(new Event('scroll'))
    expect(fade.attributes('data-transcript-fade')).toBe('visible')
  })
})
