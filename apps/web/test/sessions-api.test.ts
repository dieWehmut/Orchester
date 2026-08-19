import type { SessionDetailDto, SessionPageDto } from '@orchester/protokoll'
import { describe, expect, it, vi } from 'vitest'

import { createSessionsApi } from '../src/api/sessions'
import type { HttpClient } from '../src/api/http'

describe('session API client', () => {
  it('builds bounded cursor requests without hand-written query strings', async () => {
    const page: SessionPageDto = { schema_version: 1, items: [], next_cursor: null }
    const get = vi.fn(async () => page)
    const api = createSessionsApi({ get } as unknown as HttpClient)

    await api.list({ limit: 10, cursor: 's-cursor' })

    expect(get).toHaveBeenCalledWith('/sessions?limit=10&cursor=s-cursor')
  })

  it('encodes opaque IDs when loading a detail record', async () => {
    const detail = {} as SessionDetailDto
    const get = vi.fn(async () => detail)
    const api = createSessionsApi({ get } as unknown as HttpClient)

    await expect(api.detail('s-a/b')).resolves.toBe(detail)
    expect(get).toHaveBeenCalledWith('/sessions/s-a%2Fb')
  })
})
