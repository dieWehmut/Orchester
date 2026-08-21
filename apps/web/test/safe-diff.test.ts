import { describe, expect, it } from 'vitest'

import { prepareDiffText } from '../src/components/changes/safe-diff'

describe('prepareDiffText', () => {
  it('normalizes accepted text without interpreting its contents', () => {
    expect(prepareDiffText('- old\r\n+ <script>alert(1)</script>\r\n')).toEqual({
      status: 'ready',
      text: '- old\n+ <script>alert(1)</script>\n',
      lineCount: 3,
      byteCount: 34,
    })
  })

  it('returns an explicit empty state for whitespace-only text', () => {
    expect(prepareDiffText(' \r\n\t')).toEqual({ status: 'empty' })
  })

  it.each([
    ['NUL bytes', 'before\0after'],
    ['unexpected controls', `before${String.fromCharCode(7)}after`],
  ])('refuses %s', (_name, text) => {
    expect(prepareDiffText(text)).toEqual({
      status: 'refused',
      reason: 'binary_or_control',
    })
  })

  it('truncates by lines and reports the original bounds', () => {
    expect(prepareDiffText('one\ntwo\nthree\nfour', { maxLines: 2, maxBytes: 100 })).toEqual({
      status: 'truncated',
      text: 'one\ntwo',
      lineCount: 2,
      byteCount: 7,
      originalLineCount: 4,
      originalByteCount: 18,
    })
  })

  it('truncates on UTF-8 code point boundaries', () => {
    expect(prepareDiffText('a你b', { maxLines: 10, maxBytes: 4 })).toEqual({
      status: 'truncated',
      text: 'a你',
      lineCount: 1,
      byteCount: 4,
      originalLineCount: 1,
      originalByteCount: 5,
    })
  })
})
