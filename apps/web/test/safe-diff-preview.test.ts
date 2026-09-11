import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import SafeDiffPreview from '../src/components/changes/SafeDiffPreview.vue'

describe('SafeDiffPreview', () => {
  it('renders an empty state when no diff text is available', () => {
    const wrapper = mount(SafeDiffPreview, { props: { text: null } })

    expect(wrapper.get('[data-diff-empty]').text()).toContain('No diff preview')
  })

  it('renders accepted input as escaped plain text', () => {
    const wrapper = mount(SafeDiffPreview, {
      props: { text: '+ <script>alert(1)</script>' },
    })

    expect(wrapper.find('script').exists()).toBe(false)
    expect(wrapper.get('[data-diff-text]').text()).toContain('<script>alert(1)</script>')
    expect(wrapper.get('[data-diff-state]').text()).toContain('Text preview')
  })

  it('shows truncation metadata when the policy bounds the content', () => {
    const wrapper = mount(SafeDiffPreview, {
      props: { text: 'one\ntwo\nthree', maxLines: 2, maxBytes: 100 },
    })

    expect(wrapper.get('[data-diff-state]').text()).toContain('Truncated')
    expect(wrapper.get('[data-diff-text]').text()).toBe('one\ntwo')
    expect(wrapper.get('[data-diff-metadata]').text()).toContain('2 of 3 lines')
  })

  it('refuses control-heavy or binary-looking content', () => {
    const wrapper = mount(SafeDiffPreview, { props: { text: 'before\0after' } })

    expect(wrapper.get('[data-diff-refused]').text()).toContain('Preview unavailable')
    expect(wrapper.find('[data-diff-text]').exists()).toBe(false)
  })
})
