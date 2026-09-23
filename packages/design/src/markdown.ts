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
  | {
      kind: 'table'
      align: MarkdownColumnAlign[]
      headers: MarkdownInline[][]
      rows: MarkdownInline[][][]
    }

/** How a table's column is aligned, as its delimiter row asked for. */
export type MarkdownColumnAlign = 'start' | 'center' | 'end'

const FENCE = /^\s*```([A-Za-z0-9+#._-]*)\s*$/
const HEADING = /^(#{1,6})\s+(.*)$/
const BULLET = /^\s{0,3}[-*+]\s+(.*)$/
const NUMBERED = /^\s{0,3}\d+[.)]\s+(.*)$/
/** A row with a pipe between cells, which is what tells it from prose. */
const TABLE_ROW = /^\s*\|(.+\|.*)\|\s*$|^\s*(\S.*\|.*\S)\s*$/
/** The row of dashes and colons that turns the row above it into a header. */
const TABLE_DELIMITER = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/

/**
 * Whether a line continues a table that has already been recognised.
 *
 * Once the header and the delimiter have said this is a table, a row is any line
 * that carries a pipe - including `| 1 |`, which has one cell and is still a row.
 * The strict test above is only for finding a table in the first place.
 */
function continuesTable(line: string): boolean {
  const value = line.trim()
  if (value.length === 0) return false
  return value.startsWith('|') ? value.length > 1 : TABLE_ROW.test(line)
}
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

/** The cells of one table row, trimmed of the pipes that carry the layout. */
function tableCells(line: string): string[] {
  const trimmed = line.trim().replace(/^\|/, '').replace(/\|$/, '')
  return trimmed.split('|').map((cell) => cell.trim())
}

/** How a delimiter cell asks its column to be aligned. */
function columnAlign(cell: string): MarkdownColumnAlign {
  const value = cell.trim()
  const left = value.startsWith(':')
  const right = value.endsWith(':')
  if (left && right) return 'center'
  if (right) return 'end'
  return 'start'
}

/**
 * A pipe table, when the lines at `start` are one.
 *
 * The delimiter row is what makes a table a table: a line of pipes on its own is
 * prose a reader happened to write with pipes in it, and rendering it as a table
 * would be inventing structure the answer did not have.
 */
function readTable(
  lines: readonly string[],
  start: number,
): { block: MarkdownBlock; next: number } | null {
  const header = lines[start] ?? ''
  const delimiter = lines[start + 1] ?? ''
  if (!TABLE_ROW.test(header) || !TABLE_DELIMITER.test(delimiter)) return null
  if (!header.includes('|')) return null

  const align = tableCells(delimiter).map(columnAlign)
  const headers = tableCells(header).map(parseInline)
  const width = headers.length
  if (width === 0) return null

  const rows: MarkdownInline[][][] = []
  let index = start + 2
  while (index < lines.length && continuesTable(lines[index] ?? '')) {
    const cells = tableCells(lines[index] ?? '')
    // A row with fewer cells than the header is padded, and a longer one is
    // trimmed: a malformed row is not a reason to lose the table around it.
    const row = Array.from({ length: width }, (_, column) => parseInline(cells[column] ?? ''))
    rows.push(row)
    index += 1
  }

  return {
    block: {
      kind: 'table',
      align: Array.from({ length: width }, (_, column) => align[column] ?? 'start'),
      headers,
      rows,
    },
    next: index,
  }
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

    const table = readTable(lines, index)
    if (table !== null) {
      flushParagraph(paragraph)
      paragraph = []
      blocks.push(table.block)
      index = table.next
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