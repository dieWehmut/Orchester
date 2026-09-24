import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import MarkdownCodeCard from '../src/components/MarkdownCodeCard.vue'

/**
 * A block of code, as the reference draws one: a card whose head names the
 * language and holds the two things a reader does to a block - wrap it, or take
 * it away.
 *
 * Wrapping belongs to the block rather than to the transcript: the block a
 * reader wants wrapped is the one whose line is running off the edge.
 */

const writes: string[] = []

beforeEach(() => {
  writes.length = 0
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn(async (text: string) => void writes.push(text)) },
  })
})

function mountCard(props: Record<string, unknown> = {}) {
  return mount(MarkdownCodeCard, {
    props: {
      text: 'New-Item -ItemType File -Force',
      language: 'powershell',
      plainLabel: 'Plain text',
      wrapLabel: 'Wrap lines',
      unwrapLabel: 'Stop wrapping',
      copyLabel: 'Copy code',
      copiedLabel: 'Copied',
      ...props,
    },
  })
}

describe('MarkdownCodeCard', () => {
  it('names the language the block was written in', () => {
    const card = mountCard()

    expect(card.get('[data-code-language-label]').text()).toBe('powershell')
  })

  it('names a block that named no language instead of leaving a gap', () => {
    const card = mountCard({ language: null })

    expect(card.get('[data-code-language-label]').text()).toBe('Plain text')
  })

  it('wraps this block, and only this block', async () => {
    const card = mountCard()
    const toggle = card.get('[data-code-wrap-toggle]')

    expect(card.attributes('data-code-wrap')).toBe('false')
    expect(toggle.attributes('aria-pressed')).toBe('false')
    expect(toggle.attributes('aria-label')).toBe('Wrap lines')

    await toggle.trigger('click')

    expect(card.attributes('data-code-wrap')).toBe('true')
    expect(toggle.attributes('aria-pressed')).toBe('true')
    expect(toggle.attributes('aria-label')).toBe('Stop wrapping')
  })

  it('copies the block, and says so while it is saying it', async () => {
    const card = mountCard()
    const copy = card.get('[data-code-copy]')

    expect(copy.attributes('aria-label')).toBe('Copy code')

    await copy.trigger('click')

    expect(writes).toEqual(['New-Item -ItemType File -Force'])
    expect(copy.attributes('aria-label')).toBe('Copied')
  })

  it('says nothing about the clipboard it cannot reach', async () => {
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: undefined })
    const card = mountCard()
    const copy = card.get('[data-code-copy]')

    // A refused clipboard is not a reason to change what the block says.
    await copy.trigger('click')

    expect(copy.attributes('aria-label')).toBe('Copy code')
    expect(writes).toEqual([])
  })
})