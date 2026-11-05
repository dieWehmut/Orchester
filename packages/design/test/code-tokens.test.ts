import { describe, expect, it } from 'vitest'
import { codeTokens } from '../src/code-tokens'
import { parseMarkdown } from '../src/markdown'
describe('conversation code and quotes', () => {
  it('highlights C++ without changing any source bytes or interpreting HTML', () => {
    const source = 'int main() {\n  // <script>alert(1)</script>\n  return 42;\n}'
    const tokens = codeTokens(source, 'cpp')
    expect(tokens.map(token => token.text).join('')).toBe(source)
    expect(tokens.some(token => token.kind === 'keyword' && token.text === 'int')).toBe(true)
    expect(tokens.some(token => token.kind === 'number' && token.text === '42')).toBe(true)
    expect(tokens.some(token => token.kind === 'comment' && token.text.includes('<script>'))).toBe(true)
  })
  it('keeps plain unlabelled code and groups quoted paragraphs', () => {
    expect(codeTokens('<img onerror=bad()>', null)).toEqual([{ text: '<img onerror=bad()>', kind: 'plain' }])
    expect(parseMarkdown('> first\n> **second**\n\nnext').map(block => block.kind)).toEqual(['quote', 'paragraph'])
  })
})
