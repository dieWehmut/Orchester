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

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
const hydratedDetail = { ...second, schema_version: 1, prompt: 'Fix', final_text: 'Done', usage: {} } as SessionDetailDto
const initialPage: SessionPageDto = { schema_version: 1, items: [first], next_cursor: first.id }

describe('session metadata hydration races', () => {
  it('retains missing-page details when pagination finishes before hydration', async () => {
    const detail = deferred<SessionDetailDto>()
    const api: SessionsApi = {
      list: vi.fn().mockResolvedValueOnce(initialPage).mockResolvedValueOnce({ ...initialPage, next_cursor: null }),
      detail: vi.fn(() => detail.promise),
    }
    const store = createSessionsStore(api)
    await store.load()
    const hydration = store.ensureItems([second.id])
    await store.loadMore()
    detail.resolve(hydratedDetail)
    await hydration
    expect(store.items.value.map(row => row.id)).toEqual([first.id, second.id])
  })

  it.each(['detail-first', 'refresh-first'])('retains hydrated rows during refresh (%s)', async order => {
    const detail = deferred<SessionDetailDto>()
    const page = deferred<SessionPageDto>()
    const api: SessionsApi = {
      list: vi.fn().mockResolvedValueOnce(initialPage).mockImplementationOnce(() => page.promise).mockResolvedValue(initialPage),
      detail: vi.fn(() => detail.promise),
    }
    const store = createSessionsStore(api)
    await store.load()
    const hydration = store.ensureItems([second.id])
    const refresh = store.load()
    if (order === 'detail-first') { detail.resolve(hydratedDetail); await hydration }
    page.resolve(initialPage)
    await refresh
    if (order === 'refresh-first') {
      await store.ensureItems([second.id])
      detail.resolve(hydratedDetail)
      await hydration
    }
    expect(store.items.value.map(row => row.id)).toEqual([first.id, second.id])
    await store.load()
    expect(store.items.value.map(row => row.id)).toEqual([first.id, second.id])
    expect(api.detail).toHaveBeenCalledTimes(1)
  })

  it('invalidates old hydration on reset and permits a fresh request for the same ID', async () => {
    const stale = deferred<SessionDetailDto>()
    const fresh = deferred<SessionDetailDto>()
    const api: SessionsApi = {
      list: vi.fn().mockResolvedValue(initialPage),
      detail: vi.fn().mockImplementationOnce(() => stale.promise).mockImplementationOnce(() => fresh.promise),
    }
    const store = createSessionsStore(api)
    await store.load()
    const oldHydration = store.ensureItems([second.id])
    store.reset()
    await store.load()
    const newHydration = store.ensureItems([second.id])
    stale.resolve(hydratedDetail)
    await oldHydration
    expect(store.items.value).toEqual([first])
    await store.ensureItems([second.id])
    expect(api.detail).toHaveBeenCalledTimes(2)
    fresh.resolve({ ...hydratedDetail, title: 'Fresh authorized detail' })
    await newHydration
    expect(store.items.value[1]?.title).toBe('Fresh authorized detail')
    store.reset()
    await store.load()
    expect(store.items.value).toEqual([first])
  })

  it('does not internally retry missing history records', async () => {
    const api: SessionsApi = {
      list: vi.fn().mockResolvedValue(initialPage),
      detail: vi.fn().mockRejectedValue({ status: 404 }),
    }
    const store = createSessionsStore(api)
    await store.load()
    await store.ensureItems([second.id])
    expect(api.detail).toHaveBeenCalledTimes(1)
    expect(store.items.value).toEqual([first])
  })
})
