import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import { CodePreview, resetAppearanceForTests } from '../src'

beforeEach(() => {
  localStorage.clear()
  resetAppearanceForTests()
})

describe('CodePreview', () => {
  it('renders the line-numbered editor the reference surface shows', () => {
    const wrapper = mount(CodePreview, {
      props: { before: ['surface: "sidebar"'], after: ['surface: "sidebar-elevated"'] },
    })

    expect(wrapper.get('[data-code-preview]')).toBeTruthy()
    expect(wrapper.findAll('[data-code-line]').length).toBeGreaterThan(0)
    expect(wrapper.get('[data-code-preview]').text()).toContain('surface')
  })

  it('marks removed lines and added lines distinctly', () => {
    const wrapper = mount(CodePreview, {
      props: { before: ['surface: "sidebar"'], after: ['surface: "sidebar-elevated"'] },
    })

    expect(wrapper.find('[data-code-line="removed"]').exists()).toBe(true)
    expect(wrapper.find('[data-code-line="added"]').exists()).toBe(true)
  })

  it('keeps the unchanged lines unmarked', () => {
    const wrapper = mount(CodePreview, {
      props: { before: ['const a = 1'], after: ['const a = 1'] },
    })

    expect(wrapper.find('[data-code-line="removed"]').exists()).toBe(false)
    expect(wrapper.find('[data-code-line="added"]').exists()).toBe(false)
  })

  it('gives each column the gutter that names its side of the change', () => {
    const wrapper = mount(CodePreview, {
      props: { before: ['surface: "sidebar"'], after: ['surface: "sidebar-elevated"'] },
    })

    // The reference paints a full-height rail down the edge of each pane, in
    // the colour of the change that pane carries, so a reader can tell which
    // side they are looking at without reading the numbers.
    expect(wrapper.get('[data-code-column="before"]').attributes('data-code-gutter')).toBe('removed')
    expect(wrapper.get('[data-code-column="after"]').attributes('data-code-gutter')).toBe('added')
  })

  it('leaves the head out when it has nothing to say', () => {
    const bare = mount(CodePreview, {
      props: { before: ['const a = 1'], after: ['const a = 2'] },
    })
    const titled = mount(CodePreview, {
      props: { before: ['const a = 1'], after: ['const a = 2'], title: 'Code preview' },
    })

    // The reference's preview is a bare pair of panes under the cards, with no
    // heading row of its own; the title is only drawn for callers that ask.
    expect(bare.find('[data-code-head]').exists()).toBe(false)
    expect(titled.get('[data-code-head]').text()).toContain('Code preview')
  })
})
