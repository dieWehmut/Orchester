import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { createEmptyRunView } from '@orchester/ereignis'
import RunPanel from '../src/components/run/RunPanel.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('RunPanel', () => {
  it('renders an actionable empty run with composer and footer', () => {
    const wrapper = mount(RunPanel, { props: { view: createEmptyRunView() } })

    expect(wrapper.get('[data-run-panel]')).toBeTruthy()
    expect(wrapper.get('[data-run-composer]')).toBeTruthy()
    expect(wrapper.get('[data-run-footer]')).toBeTruthy()
    expect(wrapper.get('[data-empty-workspace]')).toBeTruthy()
    expect(wrapper.get('[data-orchester-mark]')).toBeTruthy()
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
})
