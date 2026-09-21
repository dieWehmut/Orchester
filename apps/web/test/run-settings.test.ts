import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'

import {
  DEFAULT_RUN_SETTINGS,
  readRunSettings,
  writeRunSettings,
  type RunSettings,
} from '../src/components/run/run-settings'

/**
 * Run-scoped composer settings, task U4-03 of the implementation plan.
 *
 * Section 4.6 is specific: model, effort and approval preset belong to the
 * task, not to the user. A resumed run has to keep the settings it was started
 * with, and starting a new task must not inherit the last one's full access.
 * That is why the settings are keyed by the task rather than stored once.
 */

const TASK = 'task-alpha'
const OTHER = 'task-beta'

function settings(overrides: Partial<RunSettings> = {}): RunSettings {
  return { ...DEFAULT_RUN_SETTINGS, ...overrides }
}

describe('run settings storage', () => {
  it('starts a task with no run of its own at the defaults', () => {
    expect(readRunSettings(TASK)).toEqual(DEFAULT_RUN_SETTINGS)
  })

  it('keeps the settings of one task apart from the settings of another', () => {
    writeRunSettings(TASK, settings({ approvalPreset: 'full-access' }))
    writeRunSettings(OTHER, settings({ approvalPreset: 'ask' }))

    expect(readRunSettings(TASK).approvalPreset).toBe('full-access')
    expect(readRunSettings(OTHER).approvalPreset).toBe('ask')
  })

  it('round-trips the model, the effort and the preset', () => {
    writeRunSettings(TASK, settings({ model: 'gpt-5.6-terra', effort: 'high', approvalPreset: 'governed' }))

    expect(readRunSettings(TASK)).toEqual({
      model: 'gpt-5.6-terra',
      effort: 'high',
      approvalPreset: 'governed',
    })
  })

  it('falls back to the defaults for a payload it did not write', () => {
    // A stored value that is not one of the presets is not a preset; opening
    // the composer on it would leave the approval control showing nothing.
    writeRunSettings(TASK, settings({ approvalPreset: 'governed' }))
    const key = 'orchester:run-settings:task-alpha'
    localStorage.setItem(key, 'not json')

    expect(readRunSettings(TASK)).toEqual(DEFAULT_RUN_SETTINGS)

    localStorage.setItem(key, JSON.stringify({ approvalPreset: 'yolo' }))
    expect(readRunSettings(TASK).approvalPreset).toBe(DEFAULT_RUN_SETTINGS.approvalPreset)
  })

  it('does not carry settings from a task nobody has opened a run in', () => {
    writeRunSettings(TASK, settings({ model: 'gpt-5.6-terra' }))
    expect(readRunSettings('task-never-run')).toEqual(DEFAULT_RUN_SETTINGS)
  })

  it('forgets the settings of a task when asked, so a fresh run starts clean', () => {
    writeRunSettings(TASK, settings({ approvalPreset: 'full-access' }))
    writeRunSettings(TASK, null)

    expect(readRunSettings(TASK)).toEqual(DEFAULT_RUN_SETTINGS)
  })
})

/**
 * The wiring: the composer is handed the settings its task was last run with,
 * and reports every change back so the task remembers it. A storage module
 * nothing calls is the arithmetic alone.
 */
describe('RunComposer run-scoped settings', () => {
  it('opens on the settings it is given rather than on a private default', () => {
    const wrapper = mount(RunComposer, {
      props: {
        modelValue: '',
        approvalPreset: 'governed',
        settingsKey: TASK,
      },
    })

    expect(wrapper.get('[data-approval-preset]').attributes('data-approval-preset-state')).toBe(
      'governed',
    )
    expect(wrapper.get('[data-approval-preset-label]').text()).toBe('Governed')
  })

  it('writes a preset change back to the task it belongs to', async () => {
    const wrapper = mount(RunComposer, {
      props: { modelValue: '', approvalPreset: 'ask', settingsKey: TASK },
    })

    await wrapper.get('[data-approval-preset-trigger]').trigger('click')
    await wrapper.findAll('[role="menuitem"]')[1]!.trigger('click')

    // The change is reported to the caller, which owns the task, and written
    // under that task's key rather than globally.
    expect(wrapper.emitted('update:approvalPreset')?.at(-1)).toEqual(['governed'])
    expect(readRunSettings(TASK).approvalPreset).toBe('governed')
    expect(readRunSettings(OTHER)).toEqual(DEFAULT_RUN_SETTINGS)
  })
})
