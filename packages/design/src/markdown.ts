/**
 * A markdown subset, parsed into tokens rather than into HTML.
 *
 * An agent's answer arrives in markdown - fenced code, inline code, lists,
 * links - and the transcript has to render it, which the reference does. The
 * parser stops at a token tree on purpose: a renderer that receives tokens
 * cannot be made to inject markup by the text it was given, and the safety
 * rules below are therefore structural rather than a sanitiser applied
 * afterwards.
 *
 * The subset is the one an answer actually uses. Anything outside it - tables,
 * nested lists, images, raw HTML - is left as the text it was written as, which
 * is a worse rendering but never a wrong one.
 */

export type MarkdownInline =
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'strong'; text: string }
  | { kind: 'link'; text: string; href: string }

export type MarkdownBlock =
  | { kind: 'paragraph'; spans: MarkdownInline[] }
  | { kind: 'heading'; level: number; spans: MarkdownInline[] }
  | { kind: 'code'; language: string | null; text: string }
  | { kind: 'list'; ordered: boolean; items: MarkdownInline[][] }

const FENCE = /^\s*```([A-Za-z0-9+#._-]*)\s*$/
const HEADING = /^(#{1,6})\s+(.*)$/
const BULLET = /^\s{0,3}[-*+]\s+(.*)$/
const NUMBERED = /^\s{0,3}\d+[.)]\s+(.*)$/
const INLINE_CODE = /`([^`\n]+)`/
const MARKDOWN_LINK = /\[([^\]\n]*)\]\(([^()\s]*)\)/
const BARE_LINK = /https?:\/\/[^\s<>()]+/
const BOLD = /\*\*([^*\n]+)\*\*/
const TRAILING_PUNCTUATION = /[.,;:!?]+$/

/**
 * Whether a browser should follow this address.
 *
 * Only http and https: an answer can contain any text at all, and `javascript:`
 * in a link is the one shape that turns reading into running. A relative address
 * is refused too - there is no page here for it to mean anything against.
 */
export function isSafeHref(href: string): boolean {
  if (!/^https?:\/\//i.test(href)) return false
  if (/[\s<>"'`]/.test(href)) return false
  return href.length > 'https://'.length
}

/** A trailing sentence mark belongs to the sentence, not to the address. */
function splitTrailingPunctuation(value: string): [string, string] {
  const match = TRAILING_PUNCTUATION.exec(value)
  if (match === null) return [value, '']
  return [value.slice(0, -match[0].length), match[0]]
}

/**
 * The spans of one line of prose.
 *
 * Inline code is read first, so markdown inside it stays literal; the rest of
 * the line is then read for links and bold. An unclosed marker is left as the
 * text it was, rather than swallowing everything after it.
 */
export function parseInline(source: string): MarkdownInline[] {
  const spans: MarkdownInline[] = []
  let rest = source

  while (rest.length > 0) {
    const code = INLINE_CODE.exec(rest)
    const link = MARKDOWN_LINK.exec(rest)
    const bare = BARE_LINK.exec(rest)
    const bold = BOLD.exec(rest)

    const candidates = [
      code === null ? null : { at: code.index, kind: 'code' as const, match: code },
      link === null ? null : { at: link.index, kind: 'link' as const, match: link },
      bare === null ? null : { at: bare.index, kind: 'bare' as const, match: bare },
      bold === null ? null : { at: bold.index, kind: 'strong' as const, match: bold },
    ].filter((candidate): candidate is NonNullable<typeof candidate> => candidate !== null)

    if (candidates.length === 0) {
      spans.push({ kind: 'text', text: rest })
      break
    }

    const next = candidates.reduce((earliest, candidate) =>
      candidate.at < earliest.at ? candidate : earliest,
    )

    if (next.at > 0) spans.push({ kind: 'text', text: rest.slice(0, next.at) })

    const matched = next.match[0]
    if (next.kind === 'code') {
      spans.push({ kind: 'code', text: next.match[1] ?? '' })
    } else if (next.kind === 'link') {
      const href = next.match[2] ?? ''
      if (isSafeHref(href)) spans.push({ kind: 'link', text: next.match[1] ?? '', href })
      else spans.push({ kind: 'text', text: matched })
    } else if (next.kind === 'bare') {
      const [address, punctuation] = splitTrailingPunctuation(matched)
      spans.push({ kind: 'link', text: address, href: address })
      if (punctuation.length > 0) spans.push({ kind: 'text', text: punctuation })
    } else {
      spans.push({ kind: 'strong', text: next.match[1] ?? '' })
    }

    rest = rest.slice(next.at + matched.length)
  }

  return spans.filter((span) => span.kind !== 'text' || span.text.length > 0)
}

/** The blocks of a whole answer. */
export function parseMarkdown(source: string): MarkdownBlock[] {
  const lines = source.replace(/\r\n?/g, '\n').split('\n')
  const blocks: MarkdownBlock[] = []
  let index = 0

  const flushParagraph = (paragraph: string[]): void => {
    if (paragraph.length === 0) return
    blocks.push({ kind: 'paragraph', spans: parseInline(paragraph.join(' ').trim()) })
  }

  let paragraph: string[] = []

  while (index < lines.length) {
    const line = lines[index] ?? ''

    const fence = FENCE.exec(line)
    if (fence !== null) {
      flushParagraph(paragraph)
      paragraph = []
      const language = (fence[1] ?? '').length > 0 ? fence[1]! : null
      const body: string[] = []
      index += 1
      while (index < lines.length && FENCE.exec(lines[index] ?? '') === null) {
        body.push(lines[index] ?? '')
        index += 1
      }
      // A fence that was never closed keeps the rest as code: the alternative
      // is dropping text the reader was sent.
      if (index < lines.length) index += 1
      blocks.push({ kind: 'code', language, text: body.join('\n') })
      continue
    }

    if (line.trim().length === 0) {
      flushParagraph(paragraph)
      paragraph = []
      index += 1
      continue
    }

    const heading = HEADING.exec(line)
    if (heading !== null) {
      flushParagraph(paragraph)
      paragraph = []
      blocks.push({
        kind: 'heading',
        level: (heading[1] ?? '').length,
        spans: parseInline(heading[2] ?? ''),
      })
      index += 1
      continue
    }

    const bullet = BULLET.exec(line)
    const numbered = NUMBERED.exec(line)
    if (bullet !== null || numbered !== null) {
      flushParagraph(paragraph)
      paragraph = []
      const ordered = numbered !== null
      const items: MarkdownInline[][] = []
      while (index < lines.length) {
        const candidate = lines[index] ?? ''
        const item = ordered ? NUMBERED.exec(candidate) : BULLET.exec(candidate)
        if (item === null) break
        items.push(parseInline(item[1] ?? ''))
        index += 1
      }
      blocks.push({ kind: 'list', ordered, items })
      continue
    }

    paragraph.push(line.trim())
    index += 1
  }

  flushParagraph(paragraph)
  return blocks
}