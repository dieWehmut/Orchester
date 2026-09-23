import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ComposerContextBar from '../src/components/run/ComposerContextBar.vue'

/**
 * The one fact about the workspace the composer states.
 *
 * The model used to be here too, as a readout. It is a control now - the model
 * the next run uses is a decision rather than a fact about the workspace - and
 * it is drawn at the trailing edge of the field, where the reference draws it.
 */
describe('ComposerContextBar', () => {
  it('shows the project the run will work in', () => {
    const wrapper = mount(ComposerContextBar, {
      props: { workspaceName: 'Orchester' },
    })

    expect(wrapper.get('[data-project-context]').text()).toContain('Orchester')
    expect(wrapper.findAll('svg').length).toBeGreaterThanOrEqual(1)
  })

  it('renders an explicit fallback while the project is unavailable', () => {
    const wrapper = mount(ComposerContextBar, { props: { workspaceName: null } })

    expect(wrapper.get('[data-project-context]').text()).toContain('Choose project')
  })

  it('does not expose a non-functional project button', () => {
    const wrapper = mount(ComposerContextBar, {
      props: { workspaceName: 'Orchester' },
    })

    // Choosing the workspace is the runtime's, not this row's: a button here
    // would be a control that cannot do anything, and the model that can be
    // chosen lives at the field's trailing edge instead.
    expect(wrapper.findAll('button')).toHaveLength(0)
    expect(wrapper.find('[data-model-context]').exists()).toBe(false)
  })
})