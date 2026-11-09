import type { SessionDetailDto, UiEventEnvelope } from '@orchester/protokoll'
import { projectRunEvents } from '@orchester/ereignis'

export function conversationUrl(id: string, base: string): string {
  const url = new URL('/workspace', base)
  url.searchParams.set('session', id)
  return url.href
}

/** Export the same settled messages that the conversation reader displays. */
export function conversationMarkdown(detail: SessionDetailDto, title = detail.title): string {
  const blocks: string[] = [`# ${title.replace(/[\r\n]+/g, ' ')}`]
  const runs = new Map<string, UiEventEnvelope[]>()
  for (const event of detail.events ?? []) {
    const group = runs.get(event.run_id) ?? []
    group.push(event)
    runs.set(event.run_id, group)
  }
  for (const events of runs.values()) {
    const view = projectRunEvents(events, { firstSequence: events[0]!.sequence })
    for (const item of view.timeline) {
      if (item.type === 'message') blocks.push(`## ${item.role === 'user' ? 'User' : 'Assistant'}\n\n${item.text}`)
    }
  }
  if (blocks.length === 1) blocks.push(`## User\n\n${detail.prompt}`, `## Assistant\n\n${detail.final_text}`)
  return blocks.join('\n\n') + '\n'
}
