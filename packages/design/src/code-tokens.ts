/** Lightweight lexical highlighting. Tokens remain plain Vue text, including HTML. */
export interface CodeToken { text: string; kind: 'plain' | 'keyword' | 'string' | 'comment' | 'number' }
const KEYWORDS = new Set('alignas auto bool break case catch char class const constexpr continue def delete do double elif else enum except export extern false final finally float for from function if import in inline int interface let long namespace new noexcept None null nullptr private protected public raise return short signed sizeof static struct super switch template this throw true try type typedef typename undefined union unsigned using var virtual void volatile while with yield async await include define print'.split(' '))
export function codeTokens(source: string, language: string | null): CodeToken[] {
  if (!language || source.length > 200_000) return [{ text: source, kind: 'plain' }]
  const pattern = /\/\*[\s\S]*?\*\/|\/\/[^\n]*|#[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b\d+(?:\.\d+)?\b|\b[A-Za-z_$][\w$]*\b/g
  const result: CodeToken[] = []; let offset = 0
  for (const match of source.matchAll(pattern)) {
    const at = match.index ?? 0; const text = match[0]
    if (at > offset) result.push({ text: source.slice(offset, at), kind: 'plain' })
    const hashComment = text.startsWith('#') && ['python', 'py', 'bash', 'sh', 'yaml', 'yml', 'ruby', 'rb'].includes(language.toLowerCase())
    const kind = text.startsWith('//') || text.startsWith('/*') || hashComment ? 'comment' : /^["'`]/.test(text) ? 'string' : /^\d/.test(text) ? 'number' : KEYWORDS.has(text) ? 'keyword' : 'plain'
    result.push({ text, kind }); offset = at + text.length
  }
  if (offset < source.length) result.push({ text: source.slice(offset), kind: 'plain' })
  return result
}
