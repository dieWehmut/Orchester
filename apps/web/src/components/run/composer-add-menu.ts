export type ComposerAddIcon = 'files' | 'workspace' | 'goal' | 'plan' | 'draw' | 'server' | 'plugin'

export interface ComposerAddItem {
  id: string
  label: string
  description?: string
  icon: ComposerAddIcon
  provider?: string
  checked?: boolean
  disabled?: boolean
}

export interface ComposerAddGroup {
  id: string
  label: string
  items: readonly ComposerAddItem[]
  emptyText?: string
}
