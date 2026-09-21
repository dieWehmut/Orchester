import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import SafeDiffPreview from '../src/components/changes/SafeDiffPreview.vue'

/** The wrap preference is a stored one, so every case starts from a clean slate. */
beforeEach(() => {
  localStorage.clear()
})

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

describe('SafeDiffPreview line wrap', () => {
  it('does not wrap lines until the reader asks it to', () => {
    // Section 4.7 asks for a line-wrap toggle on diffs. A diff is read in
    // columns, so unwrapped is the state a reader expects first.
    const wrapper = mount(SafeDiffPreview, { props: { text: "+ a very long line" } })

    expect(wrapper.get('[data-diff-text]').attributes('data-diff-wrap')).toBe('off')
  })

  it('wraps lines when the toggle is on', async () => {
    const wrapper = mount(SafeDiffPreview, { props: { text: "+ a very long line" } })

    await wrapper.get('[data-diff-wrap-toggle]').trigger('click')

    expect(wrapper.get('[data-diff-text]').attributes('data-diff-wrap')).toBe('on')
  })

  it('turns wrapping back off', async () => {
    const wrapper = mount(SafeDiffPreview, { props: { text: "+ a very long line" } })

    await wrapper.get('[data-diff-wrap-toggle]').trigger('click')
    await wrapper.get('[data-diff-wrap-toggle]').trigger('click')

    expect(wrapper.get('[data-diff-text]').attributes('data-diff-wrap')).toBe('off')
  })

  it('reports the toggle state to assistive technology', async () => {
    // The contract in section 7 requires every control to be reachable and
    // named; a pressed state that is only visual is a state a keyboard user
    // cannot confirm.
    const wrapper = mount(SafeDiffPreview, { props: { text: "+ a very long line" } })
    const toggle = wrapper.get('[data-diff-wrap-toggle]')

    expect(toggle.attributes('aria-pressed')).toBe('false')
    expect(toggle.attributes('aria-label')).toBeTruthy()

    await toggle.trigger('click')
    expect(toggle.attributes('aria-pressed')).toBe('true')
  })

  it('remembers the choice for the next diff', async () => {
    // The preference belongs to the reader, not to one file: re-deciding it for
    // every diff in a long change set is exactly the friction a toggle exists
    // to remove.
    const first = mount(SafeDiffPreview, { props: { text: "+ one" } })
    await first.get('[data-diff-wrap-toggle]').trigger('click')

    const second = mount(SafeDiffPreview, { props: { text: "+ two" } })

    expect(second.get('[data-diff-text]').attributes('data-diff-wrap')).toBe('on')
  })
})
