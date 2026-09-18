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
})
