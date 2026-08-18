import { describe, expect, it } from 'vitest'

import { AppDialog, AppDrawer, AppPopover } from '../src'

describe('overlay public API', () => {
  it('exports the three overlay primitives', () => {
    expect([AppDialog, AppDrawer, AppPopover]).not.toContain(undefined)
  })
})
