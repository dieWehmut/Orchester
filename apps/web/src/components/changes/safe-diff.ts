export interface DiffTextLimits {
  readonly maxBytes?: number
  readonly maxLines?: number
}

export type PreparedDiffText =
  | { readonly status: 'empty' }
  | { readonly status: 'refused'; readonly reason: 'binary_or_control' }
  | {
      readonly status: 'ready'
      readonly text: string
      readonly lineCount: number
      readonly byteCount: number
    }
  | {
      readonly status: 'truncated'
      readonly text: string
      readonly lineCount: number
      readonly byteCount: number
      readonly originalLineCount: number
      readonly originalByteCount: number
    }

const DEFAULT_MAX_BYTES = 256 * 1024
const DEFAULT_MAX_LINES = 2_000
const UNSAFE_CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u
const encoder = new TextEncoder()

function positiveLimit(value: number | undefined, fallback: number): number {
  return value === undefined || !Number.isFinite(value) || value < 1
    ? fallback
    : Math.floor(value)
}

function byteLength(value: string): number {
  return encoder.encode(value).byteLength
}

function truncateUtf8(value: string, maxBytes: number): string {
  let used = 0
  let output = ''

  for (const character of value) {
    const size = byteLength(character)
    if (used + size > maxBytes) break
    output += character
    used += size
  }

  return output
}

function lineCount(value: string): number {
  return value.split('\n').length
}

export function prepareDiffText(
  raw: string,
  limits: DiffTextLimits = {},
): PreparedDiffText {
  const normalized = raw.replace(/\r\n?/gu, '\n')
  if (normalized.trim().length === 0) return { status: 'empty' }
  if (UNSAFE_CONTROL.test(normalized)) {
    return { status: 'refused', reason: 'binary_or_control' }
  }

  const maxBytes = positiveLimit(limits.maxBytes, DEFAULT_MAX_BYTES)
  const maxLines = positiveLimit(limits.maxLines, DEFAULT_MAX_LINES)
  const originalLineCount = lineCount(normalized)
  const originalByteCount = byteLength(normalized)
  let text = normalized

  if (originalLineCount > maxLines) text = text.split('\n').slice(0, maxLines).join('\n')
  if (byteLength(text) > maxBytes) text = truncateUtf8(text, maxBytes)

  const prepared = {
    text,
    lineCount: lineCount(text),
    byteCount: byteLength(text),
  }

  if (text === normalized) return { status: 'ready', ...prepared }
  return {
    status: 'truncated',
    ...prepared,
    originalLineCount,
    originalByteCount,
  }
}
