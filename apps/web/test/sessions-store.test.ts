import type { SessionDetailDto, SessionPageDto, SessionSummaryDto } from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import type { SessionsApi } from '../src/api/sessions'
import { createSessionsStore } from '../src/stores/sessions'

const first: SessionSummaryDto = {
  id: 's-11111111111111111111111111111111',
  source: 'delegate',
  recorded_at_unix: 1_700_000_001,
  title: 'Inspect the runtime',
  agent: 'codex',
  model: 'gpt-5',
  outcome: 'success',
  resumable: true,
}

const second: SessionSummaryDto = {
  ...first,
  id: 's-22222222222222222222222222222222',
  title: 'Repair the tests',
}

describe('sessions store', () => {
  it('loads pages, appends unique records, and selects details', async () => {
    const pages: SessionPageDto[] = [
      { schema_version: 1, items: [first], next_cursor: first.id },
      { schema_version: 1, items: [first, second], next_cursor: null },
    ]
    const detail = { ...second, schema_version: 1, prompt: 'Fix', final_text: 'Done', usage: {} } as SessionDetailDto
    const api: SessionsApi = {
      list: vi.fn(async () => pages.shift() as SessionPageDto),
      detail: vi.fn(async () => detail),
    }
    const store = createSessionsStore(api)

    await store.load()
    await store.loadMore()
    await store.select(second.id)

    expect(store.items.value.map((item) => item.id)).toEqual([first.id, second.id])
    expect(store.nextCursor.value).toBeNull()
    expect(store.selected.value).toBe(detail)
    expect(store.status.value).toBe('ready')
  })

  it('keeps loaded rows when a refresh fails', async () => {
    const api: SessionsApi = {
      list: vi
        .fn()
        .mockResolvedValueOnce({ schema_version: 1, items: [first], next_cursor: null })
        .mockRejectedValueOnce(new TypeError('Failed to fetch')),
      detail: vi.fn(),
    }
    const store = createSessionsStore(api)

    await store.load()
    await store.load()

    expect(store.items.value).toEqual([first])
    expect(store.status.value).toBe('error')
    expect(store.error.value).toMatchObject({ code: 'network', retryable: true })
  })
})
