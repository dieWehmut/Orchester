export type InspectorTab = 'context' | 'approvals' | 'changes'

export function isInspectorTab(value: string): value is InspectorTab {
  return value === 'context' || value === 'approvals' || value === 'changes'
}
