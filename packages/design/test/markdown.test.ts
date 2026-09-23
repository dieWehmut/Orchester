import { describe, expect, it } from 'vitest'

import { parseMarkdown } from '../src/markdown'

/**
 * The markdown subset a transcript renders.
 *
 * An answer from a coding agent is written in markdown - fenced code, inline
 * code, lists, links - and the reference renders it. This parser keeps the part
 * that carries meaning and refuses the part that carries risk: it produces a
 * token tree rather than HTML, so a caller cannot inject markup by writing it,
 * and a link is only a link when it goes somewhere a browser should follow.
 */

describe('markdown blocks', () => {
  it('separates paragraphs and treats a single newline as a soft break', () => {
    const blocks = parseMarkdown('One line\nstill the same paragraph.\n\nSecond paragraph.')

    expect(blocks).toHaveLength(2)
    expect(blocks[0]).toEqual({
      kind: 'paragraph',
      spans: [
        { kind: 'text', text: 'One line still the same paragraph.' },
      ],
    })
    expect(blocks[1]).toMatchObject({ kind: 'paragraph' })
  })

  it('reads a fenced block as code, with its language', () => {
    const blocks = parseMarkdown('Before.\n\n```ts\nconst a = 1\n```\n\nAfter.')

    expect(blocks.map((block) => block.kind)).toEqual(['paragraph', 'code', 'paragraph'])
    expect(blocks[1]).toEqual({ kind: 'code', language: 'ts', text: 'const a = 1' })
  })

  it('keeps an unterminated fence as code rather than losing the rest', () => {
    const blocks = parseMarkdown('```\nconst a = 1\nconst b = 2')

    expect(blocks).toEqual([{ kind: 'code', language: null, text: 'const a = 1\nconst b = 2' }])
  })

  it('never reads markdown inside a code block', () => {
    const [block] = parseMarkdown('```\n**not bold** [x](https://example.com)\n```')

    expect(block).toMatchObject({ kind: 'code' })
    expect(block?.kind === 'code' ? block.text : '').toContain('**not bold**')
  })

  it('reads headings and both kinds of list', () => {
    const [heading] = parseMarkdown('## Summary')
    const [bullets] = parseMarkdown('- one\n- two')
    const [numbered] = parseMarkdown('1. one\n2. two')

    expect(heading).toMatchObject({ kind: 'heading', level: 2 })
    expect(bullets).toMatchObject({ kind: 'list', ordered: false })
    expect(numbered).toMatchObject({ kind: 'list', ordered: true })
    expect(bullets?.kind === 'list' ? bullets.items : []).toHaveLength(2)
  })

  it('reads a pipe table with its header, alignment and rows', () => {
    const [table] = parseMarkdown(
      ['| Option | Default | Meaning |', '| :--- | :---: | ---: |', '| `a` | 1 | first |'].join(
        '\n',
      ),
    )

    expect(table).toMatchObject({ kind: 'table', align: ['start', 'center', 'end'] })
    if (table?.kind !== 'table') throw new Error('expected a table')
    expect(table.headers.flat().map((span) => (span.kind === 'text' ? span.text : ''))).toEqual([
      'Option',
      'Default',
      'Meaning',
    ])
    expect(table.rows).toHaveLength(1)
    // Cells are spans like every other text in the answer: a table is another
    // arrangement of prose, not a place where markup starts being trusted.
    expect(table.rows[0]?.[0]).toEqual([{ kind: 'code', text: 'a' }])
    expect(table.rows[0]?.[2]).toEqual([{ kind: 'text', text: 'first' }])
  })

  it('pads and trims a row whose cells do not match the header', () => {
    const [table] = parseMarkdown(['| a | b | c |', '| --- | --- | --- |', '| 1 |'].join('\n'))

    if (table?.kind !== 'table') throw new Error('expected a table')
    expect(table.rows[0]).toHaveLength(3)
    expect(table.rows[0]?.[1]).toEqual([])
  })

  it('leaves a pipe line without a delimiter row as the paragraph it was', () => {
    const [alone] = parseMarkdown('| not | a table |')
    const [two] = parseMarkdown('| not | a table |\n| either |')

    expect(alone).toMatchObject({ kind: 'paragraph' })
    expect(two).toMatchObject({ kind: 'paragraph' })
  })
})

describe('markdown inline', () => {
  function spans(source: string) {
    const [block] = parseMarkdown(source)
    return block?.kind === 'paragraph' || block?.kind === 'heading' ? block.spans : []
  }

  it('reads inline code and bold', () => {
    expect(spans('Use `main` for **this** now.')).toEqual([
      { kind: 'text', text: 'Use ' },
      { kind: 'code', text: 'main' },
      { kind: 'text', text: ' for ' },
      { kind: 'strong', text: 'this' },
      { kind: 'text', text: ' now.' },
    ])
  })

  it('reads a link a browser should follow', () => {
    expect(spans('See [the pull request](https://example.com/pr/1).')).toEqual([
      { kind: 'text', text: 'See ' },
      { kind: 'link', text: 'the pull request', href: 'https://example.com/pr/1' },
      { kind: 'text', text: '.' },
    ])
  })

  it('reads a bare address as a link too', () => {
    expect(spans('At https://example.com/pr/1 today.')).toEqual([
      { kind: 'text', text: 'At ' },
      { kind: 'link', text: 'https://example.com/pr/1', href: 'https://example.com/pr/1' },
      { kind: 'text', text: ' today.' },
    ])
  })

  it('refuses a link that is not an address', () => {
    // The reading is "this is text", not "this is a link we sanitised": a
    // caller that renders the tokens cannot turn it into one.
    for (const source of [
      '[click](javascript:alert(1))',
      '[click](data:text/html;base64,PHNjcmlwdD4=)',
      '[click](file:///etc/passwd)',
    ]) {
      expect(spans(source).every((span) => span.kind !== 'link')).toBe(true)
    }
  })

  it('leaves markup in the source as text', () => {
    const source = '<img src=x onerror="alert(1)"> and <script>alert(2)</script>'

    expect(spans(source)).toEqual([{ kind: 'text', text: source }])
  })

  it('treats an unclosed marker as text rather than swallowing the rest', () => {
    expect(spans('an `unclosed backtick stays words')).toEqual([
      { kind: 'text', text: 'an `unclosed backtick stays words' },
    ])
    expect(spans('a **bold start with no end')).toEqual([
      { kind: 'text', text: 'a **bold start with no end' },
    ])
  })
})