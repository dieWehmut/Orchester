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
  disabled?: boolean
}
