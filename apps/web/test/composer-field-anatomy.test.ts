import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RunComposer from '../src/components/run/RunComposer.vue'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

/**
 * The composer, as the reference draws it.
 *
 * The reference shows one rounded field: a placeholder that names it, the
 * controls it needs at the trailing edge, and a round action that points the way
 * it sends. It prints no heading over the field and no tally under it, because
 * both would be rows the reader pays for on every prompt.
 */

function mountComposer(props: Record<string, unknown> = {}) {
  return mount(RunComposer, {
    props: {
      modelValue: 'Ship it',
      workspaceName: 'Orchester',
      modelCatalog: MODEL_CATALOG_FIXTURE,
      modelStatus: 'ready',
      ...props,
    },
  })
}

describe('composer field anatomy', () => {
  it('names the field without printing a heading over it', () => {
    const wrapper = mountComposer()

    // The placeholder says what the field is for; the accessible name is where
    // the visible label went, so the field is still named for a screen reader.
    expect(wrapper.find('label').exists()).toBe(false)
    expect(wrapper.get('textarea').attributes('aria-label')).toBe('Task prompt')
  })

  it('keeps the input and its controls inside one field', () => {
    const wrapper = mountComposer()

    const field = wrapper.get('[data-composer-field]')

    expect(field.find('textarea').exists()).toBe(true)
    expect(field.find('[data-composer-footer]').exists()).toBe(true)
    // The context row stays outside and above it: which workspace and model a
    // run will use is a fact about the run, not a control inside the field.
    expect(field.find('[data-composer-context]').exists()).toBe(false)
  })

  it('offers a round action that points the way it goes', async () => {
    const wrapper = mountComposer()

    const send = wrapper.get('[data-composer-action="submit"]')
    expect(send.attributes('data-composer-action-shape')).toBe('send')
    // The glyph is for the eye; the word is still what a reader hears.
    expect(send.attributes('aria-label')).toBe('Run')

    await wrapper.setProps({ busy: true })

    const stop = wrapper.get('[data-composer-action="cancel"]')
    expect(stop.attributes('data-composer-action-shape')).toBe('stop')
    expect(stop.attributes('aria-label')).toBe('Stop')
  })

  it('counts the characters only once they are worth counting', async () => {
    const wrapper = mountComposer({ modelValue: 'short', maxLength: 100 })

    expect(wrapper.find('[data-composer-count]').exists()).toBe(false)

    await wrapper.setProps({ modelValue: 'x'.repeat(85) })

    expect(wrapper.get('[data-composer-count]').text()).toContain('85')
  })
})