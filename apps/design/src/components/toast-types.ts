export type ToastTone = 'info' | 'success' | 'warning' | 'error'

export interface ToastItem {
  id: string
  title?: string
  message: string
  tone?: ToastTone
  timeout?: number
}
