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
    await wrapper.get('button').trigger('click')
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
})
