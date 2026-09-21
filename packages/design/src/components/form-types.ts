export interface AppFieldControlProps {
  id: string
  describedBy?: string
  invalid: boolean
  required: boolean
}

export interface AppTabOption {
  id: string
  label: string
  disabled?: boolean
}

export interface AppSelectOption {
  value: string
  label: string
  disabled?: boolean
}

export interface AppSegmentOption {
  id: string
  label: string
  disabled?: boolean
}

export interface AppMenuItem {
  id: string
  label: string
  /**
   * The chord that also reaches this item, already rendered for the platform.
   *
   * The reference prints it at the row's trailing edge, so the menu teaches the
   * shortcut instead of only offering the pointer path. It is a string rather
   * than a key list because the shell that owns the binding is the one that
   * knows how this platform spells the modifier.
   */
  hint?: string
  disabled?: boolean
}
