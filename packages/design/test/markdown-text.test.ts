import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import MarkdownText from '../src/components/MarkdownText.vue'

/**
 * The answer's renderer.
 *
 * Two things are being pinned here. The first is that markdown reaches the page
 * as elements: code is a `pre`, a list is a list, and what was written is what
 * the reader sees. The second is the guarantee that makes it safe to render text
 * an agent wrote - the component has no string of markup to interpolate, which
 * is what the "no v-html" case below asserts at the source.
 */

const source = readFileSync(
  resolve(process.cwd(), 'src/components/MarkdownText.vue'),
  'utf8',
)

describe('MarkdownText', () => {
  it('draws prose, code and bold as elements', () => {
    const wrapper = mount(MarkdownText, {
      props: { text: 'Use `main` for **this** now.' },
    })

    expect(wrapper.get('[data-markdown-paragraph]').text()).toContain('Use')
    expect(wrapper.get('[data-markdown-inline-code]').text()).toBe('main')
    expect(wrapper.get('[data-markdown-strong]').text()).toBe('this')
  })

  it('draws a fenced block as a card, with the language it was written in', () => {
    const wrapper = mount(MarkdownText, {
      props: { text: '```ts\nconst a = 1\n```', codePlainLabel: 'Plain text' },
    })

    const card = wrapper.get('[data-markdown-code]')

    // The card's head names the language and holds the block's actions; the code
    // itself is the body under them.
    expect(card.attributes('data-code-language')).toBe('ts')
    expect(card.get('[data-code-head]').text()).toContain('ts')
    expect(card.get('[data-code-body]').element.tagName).toBe('PRE')
    expect(card.get('[data-code-body]').text()).toBe('const a = 1')
  })

  it('names a fence that named no language rather than leaving the head blank', () => {
    const wrapper = mount(MarkdownText, {
      props: { text: '```\nplain\n```', codePlainLabel: 'Plain text' },
    })

    const head = wrapper.get('[data-markdown-code] [data-code-head]')

    expect(head.text()).toContain('Plain text')
  })

  it('draws a list as the list it was written as', () => {
    const bullets = mount(MarkdownText, { props: { text: '- one\n- two' } })
    const numbered = mount(MarkdownText, { props: { text: '1. one\n2. two' } })

    expect(bullets.get('[data-markdown-list]').element.tagName).toBe('UL')
    expect(numbered.get('[data-markdown-list]').element.tagName).toBe('OL')
    expect(bullets.findAll('li')).toHaveLength(2)
  })

  it('draws a table with its header, its alignment and its cells', () => {
    const wrapper = mount(MarkdownText, {
      props: {
        text: ['| Option | Default |', '| :--- | ---: |', '| `a` | 1 |'].join('\n'),
      },
    })

    const table = wrapper.get('[data-markdown-table]')

    expect(table.findAll('th')).toHaveLength(2)
    expect(table.findAll('td')).toHaveLength(2)
    expect(table.findAll('th')[1]!.attributes('style')).toContain('end')
    // Cells hold the same spans as any other text, so an inline code chip in a
    // table is the chip it is everywhere else.
    expect(table.get('[data-markdown-inline-code]').text()).toBe('a')
  })

  it('lets a table wider than the prose scroll on its own', () => {
    const wrapper = mount(MarkdownText, {
      props: { text: ['| a | b |', '| --- | --- |', '| 1 | 2 |'].join('\n') },
    })

    // The transcript must not scroll sideways because one answer held a table.
    expect(wrapper.get('[data-markdown-table-scroll]')).toBeTruthy()
  })

  it('sends a link out of the page safely, and says that it does', () => {
    const wrapper = mount(MarkdownText, {
      props: { text: 'See [the pull request](https://example.com/pr/1).', externalLabel: 'Opens in a new tab' },
    })

    const link = wrapper.get('[data-markdown-link]')

    expect(link.attributes('href')).toBe('https://example.com/pr/1')
    expect(link.attributes('target')).toBe('_blank')
    expect(link.attributes('rel')).toBe('noopener noreferrer')
    expect(link.text()).toContain('Opens in a new tab')
  })

  it('draws markup in the source as the text it was', () => {
    const wrapper = mount(MarkdownText, {
      props: { text: '<img src=x onerror="alert(1)"> and <script>alert(2)</script>' },
    })

    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.find('script').exists()).toBe(false)
    expect(wrapper.text()).toContain('<img src=x onerror="alert(1)">')
  })

  it('draws a refused link as text', () => {
    const wrapper = mount(MarkdownText, { props: { text: '[click](javascript:alert(1))' } })

    expect(wrapper.find('[data-markdown-link]').exists()).toBe(false)
    expect(wrapper.text()).toContain('click')
  })

  it('never interpolates markup, which is what makes the text safe to render', () => {
    // The comment above the component names the directive it refuses, so the
    // assertion looks for a use of it rather than the word.
    expect(source).not.toMatch(/v-html\s*=/)
  })
})