export interface AppFieldControlProps {
  id: string
  describedBy?: string
  invalid: boolean
  required: boolean
}

export interface AppSelectOption {
  value: string
  label: string
  disabled?: boolean
}
