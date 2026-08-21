import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ChangeInspector from '../src/components/changes/ChangeInspector.vue'
import type { ChangeSummary } from '../src/components/changes/change-summary'

const changes: ChangeSummary[] = [
  {
    path: 'src/app.ts',
    kind: 'update',
    latestSequence: 7,
    latestOccurredAt: '2026-08-21T00:00:07Z',
    eventCount: 2,
    history: [],
  },
  {
    path: 'src/new-file.ts',
    kind: 'add',
    latestSequence: 6,
    latestOccurredAt: '2026-08-21T00:00:06Z',
    eventCount: 1,
    history: [],
  },
]

describe('ChangeInspector', () => {
  it('renders an empty state when no change events are available', () => {
    const wrapper = mount(ChangeInspector, { props: { changes: [] } })

    expect(wrapper.get('[data-change-empty]').text()).toContain('No file changes')
  })

  it('renders real paths and event metadata in selectable buttons', async () => {
    const wrapper = mount(ChangeInspector, {
      props: { changes, selectedPath: 'src/app.ts' },
    })
    const rows = wrapper.findAll('[data-change-path]')

    expect(rows).toHaveLength(2)
    expect(rows[0]?.attributes('data-change-kind')).toBe('update')
    expect(rows[0]?.attributes('aria-pressed')).toBe('true')
    expect(rows[0]?.text()).toContain('src/app.ts')
    expect(rows[0]?.text()).toContain('2 events')
    expect(rows[0]?.text()).toContain('#7')

    await rows[1]?.trigger('click')

    expect(wrapper.emitted('select')).toEqual([['src/new-file.ts']])
  })

  it('provides a textual status in addition to the icon', () => {
    const wrapper = mount(ChangeInspector, { props: { changes } })

    expect(wrapper.get('[data-change-path="src/app.ts"]').text()).toContain('Modified')
    expect(wrapper.get('[data-change-path="src/new-file.ts"]').text()).toContain('Added')
  })
})
