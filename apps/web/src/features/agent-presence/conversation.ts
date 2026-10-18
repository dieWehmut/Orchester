import type { ActivityEntry, ActivityTurn } from '../../api/agent-activity'

export type ConversationItem =
  | { id: string; kind: 'user' | 'assistant' | 'status'; entry: ActivityEntry }
  | { id: string; kind: 'work'; entries: ActivityEntry[]; turnId: string | undefined; startedAt: number | undefined; duration: number | undefined }

/** Keep source order and stable keys, including legacy and truncated transcripts. */
export function conversationItems(entries: ActivityEntry[], turns: ActivityTurn[] = []): ConversationItem[] {
  const work = new Set<string>()
  let toolAhead = false
  let turnAhead: string | undefined
  for (let index = entries.length - 1; index >= 0; index--) {
    const entry = entries[index]!
    if (entry.turn_id !== turnAhead || entry.kind === 'user' || entry.phase === 'final') toolAhead = false
    turnAhead = entry.turn_id
    if (entry.kind === 'tool' || entry.kind === 'result') { toolAhead = true; work.add(entry.id) }
    else if (entry.kind === 'assistant' && (entry.phase === 'commentary' || (entry.phase === undefined && toolAhead))) work.add(entry.id)
  }
  const items: ConversationItem[] = []
  for (const entry of entries) {
    if (!work.has(entry.id)) {
      items.push({ id: entry.id, kind: entry.kind as 'user' | 'assistant' | 'status', entry })
      continue
    }
    const previous = items.at(-1)
    if (previous?.kind === 'work' && previous.turnId === entry.turn_id) {
      previous.entries.push(entry)
    }
    else {
      const turn = turns.find(turn => turn.id === entry.turn_id)
      const duration = turn?.started_at !== undefined && turn.completed_at !== undefined && turn.completed_at >= turn.started_at
        ? turn.completed_at - turn.started_at : undefined
      items.push({ id: `work-${entry.id}`, kind: 'work', entries: [entry], turnId: entry.turn_id, startedAt: turn?.started_at, duration })
    }
  }
  return items
}

export type WorkStep = { id: string; entry: ActivityEntry; results: ActivityEntry[] }
/** Pair by source call identity, including interleaved concurrent tools. */
export function workSteps(entries: ActivityEntry[]): WorkStep[] {
  const steps: WorkStep[] = []
  for (const entry of entries) {
    const previous = steps.at(-1)
    const invocation = entry.kind === 'result' ? entry.call_id
      ? steps.find(step => step.entry.kind === 'tool' && step.entry.call_id === entry.call_id)
      : previous?.entry.kind === 'tool' && !previous.entry.call_id ? previous : undefined
      : undefined
    if (invocation) invocation.results.push(entry)
    else steps.push({ id: entry.id, entry, results: [] })
  }
  return steps
}

export function toolPresentation(entry: ActivityEntry): 'command' | 'edit' | 'read' | 'search' | 'image' | 'integration' | 'tool' {
  const name = entry.tool?.toLowerCase() ?? ''
  if (/apply_patch|write|edit/.test(name) || /apply_patch/.test(entry.text)) return 'edit'
  if (/view_image|screenshot/.test(name)) return 'image'
  if (/search|browse|web/.test(name)) return 'search'
  if (/read|open_file/.test(name)) return 'read'
  if (/mcp__/.test(name)) return 'integration'
  if (/exec|shell|bash|terminal|command/.test(name)) return 'command'
  return 'tool'
}

/** Keep the integration's identity visible without exposing its transport prefix. */
export function toolIntegration(entry: ActivityEntry): string | null {
  const source = /(?:^|\.)mcp__([^_]+(?:_(?!_)[^_]+)*)__/i.exec(entry.tool ?? '')?.[1]
  if (!source) return null
  const names: Record<string, string> = { playwright: 'Playwright', github: 'GitHub', node_repl: 'Node', codex_app: 'Codex' }
  return names[source] ?? source.replaceAll('_', ' ')
}

/** Codex's citation bookkeeping is not part of the visible answer. Keep code literal. */
export function visibleAnswer(text: string): string {
  const citation = /^<oai-mem-citation>[\s\S]*?<\/oai-mem-citation>\s*$/m.exec(text)
  if (!citation || citation.index + citation[0].length !== text.length) return text
  const prefix = text.slice(0, citation.index)
  if ((prefix.match(/^\s*```/gm)?.length ?? 0) % 2) return text
  return prefix.trimEnd()
}

export function elapsedLabel(milliseconds: number): string {
  const seconds = Math.floor(milliseconds / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  return hours ? `${hours}h ${minutes % 60}m ${seconds % 60}s` : minutes ? `${minutes}m ${seconds % 60}s` : `${seconds}s`
}
