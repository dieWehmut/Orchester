import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import WorkspaceShell from '../src/components/layout/WorkspaceShell.vue'

describe('WorkspaceShell', () => {
  it('keeps sessions, transcript, and inspector as explicit landmark regions', () => {
    const wrapper = mount(WorkspaceShell, {
      slots: {
        sessions: '<p>Sessions</p>',
        default: '<p>Transcript</p>',
        inspector: '<p>Inspector</p>',
      },
    })

    expect(wrapper.get('[data-pane="sessions"]').attributes('aria-label')).toBe('Sessions')
    expect(wrapper.get('[data-pane="transcript"]').attributes('aria-label')).toBe('Run transcript')
    expect(wrapper.get('[data-pane="inspector"]').attributes('aria-label')).toBe('Inspector')
  })
})
