import type { RunView } from '@orchester/ereignis'

/**
 * The conversation as markdown, which is what this product's share control can
 * honestly produce.
 *
 * The reference's share publishes the thread and hands back a link; there is no
 * host here to publish to, so what leaves is text. It is written the way the
 * composer quotes a turn - the reader's words as a quotation, the answer as
 * prose - so that pasting it anywhere reads as a conversation rather than as two
 * blobs, and the run's title travels with it as the heading.
 */
export function conversationMarkdown(view: RunView): string {
  const parts: string[] = []
  const title = view.title?.trim() ?? ''
  if (title.length > 0) parts.push(`# ${title}`, '')
  for (const item of view.timeline) {
    // Only the conversation leaves: a tool card or a reasoning row is the run's
    // own record, and a reader sharing a thread is sharing what was said.
    if (item.type !== 'message') continue
    if (item.role === 'user') {
      parts.push(
        item.text
          .split('\n')
          .map((line) => (line.length > 0 ? `> ${line}` : '>'))
          .join('\n'),
        '',
      )
    } else {
      parts.push(item.text, '')
    }
  }
  return parts.join('\n').trim()
}