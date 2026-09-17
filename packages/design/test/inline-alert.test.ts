import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'

import InlineAlert from '../src/components/InlineAlert.vue'

describe('InlineAlert', () => {
  it('uses an alert live region for error messages', () => {
    const wrapper = mount(InlineAlert, {
      props: { tone: 'error', title: 'Connection failed' },
      slots: { default: 'The server did not respond.' },
    })

    expect(wrapper.attributes('role')).toBe('alert')
    expect(wrapper.attributes('aria-live')).toBe('assertive')
    expect(wrapper.text()).toContain('Connection failed')
    expect(wrapper.text()).toContain('The server did not respond.')
  })

  it('uses a polite status region and emits dismiss', async () => {
    const wrapper = mount(InlineAlert, {
      props: { tone: 'success', dismissible: true, dismissLabel: 'Dismiss notice' },
      slots: { default: 'Saved.' },
    })

    expect(wrapper.attributes('role')).toBe('status')
    expect(wrapper.attributes('aria-live')).toBe('polite')
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('dismiss')).toHaveLength(1)
  })
})
