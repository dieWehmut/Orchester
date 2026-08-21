import { describe, expect, it } from 'vitest'

import {
  AppMenu,
  AppSegmentedControl,
  AppTabs,
  AppTooltip,
  VisuallyHidden,
  type AppMenuItem,
  type AppSegmentOption,
  type AppTabOption,
} from '../src'

describe('navigation public API', () => {
  it('exports every navigation primitive and its option contracts', () => {
    const menuItem: AppMenuItem = { id: 'rename', label: 'Rename' }
    const segment: AppSegmentOption = { id: 'run', label: 'Run' }
    const tab: AppTabOption = { id: 'files', label: 'Files' }

    expect([AppMenu, AppSegmentedControl, AppTabs, AppTooltip, VisuallyHidden]).not.toContain(
      undefined,
    )
    expect([menuItem.id, segment.id, tab.id]).toEqual(['rename', 'run', 'files'])
  })
})
