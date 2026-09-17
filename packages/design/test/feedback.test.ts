import { describe, expect, it } from 'vitest'

import {
  EmptyState,
  InlineAlert,
  ProgressBar,
  SkeletonBlock,
  ToastRegion,
  type ToastItem,
  type ToastTone,
} from '../src'

describe('feedback public API', () => {
  it('exports feedback primitives and toast contracts', () => {
    const tone: ToastTone = 'warning'
    const item: ToastItem = { id: 'approval', message: 'Approval required', tone }

    expect([EmptyState, InlineAlert, ProgressBar, SkeletonBlock, ToastRegion]).not.toContain(
      undefined,
    )
    expect(item.tone).toBe('warning')
  })
})
