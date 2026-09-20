/**
 * The unified tab strip's list rules, section 4.2 of the design spec.
 *
 * The strip is the one place a task, a terminal, a diff or an agent appears as
 * a tab, so what a tab *is* belongs beside the rules that move them around
 * rather than inside the component that draws them. These helpers are pure:
 * cycling, selecting by position and reordering are decisions about an
 * ordered list, and each one is easier to hold to a rule when it is not also
 * touching the DOM.
 */

export type ShellTabKind = 'run' | 'terminal' | 'diff' | 'agent'

export interface ShellTab {
  id: string
  kind: ShellTabKind
  label: string
}

/**
 * The next tab in the strip, wrapping at both ends.
 *
 * A cycle that dead-ends at the last tab is a cycle a user has to reverse, and
 * the strip is deliberately short - the keys exist so the far end is one press
 * away from the near one.
 */
export function cycleTab(tabs: readonly ShellTab[], activeId: string, backwards = false): string | null {
  if (tabs.length === 0) return null
  const current = tabs.findIndex((tab) => tab.id === activeId)
  // An active id the strip does not hold means the strip has no selection to
  // advance from, so the cycle starts at the near end of the direction.
  if (current < 0) return (backwards ? tabs[tabs.length - 1]!.id : tabs[0]!.id)
  const step = backwards ? -1 : 1
  return tabs[(current + step + tabs.length) % tabs.length]!.id
}

/** The tab at a one-based position, as ⌘/Ctrl+1..9 addresses them. */
export function tabAt(tabs: readonly ShellTab[], position: number): string | null {
  if (position < 1 || position > 9) return null
  return tabs[position - 1]?.id ?? null
}

/** The list with one tab moved to another's place, as a drag reorder means it. */
export function reorderTab(
  tabs: readonly ShellTab[],
  fromId: string,
  toId: string,
): readonly ShellTab[] {
  const from = tabs.findIndex((tab) => tab.id === fromId)
  const to = tabs.findIndex((tab) => tab.id === toId)
  if (from < 0 || to < 0 || from === to) return tabs
  const next = [...tabs]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved!)
  return next
}
