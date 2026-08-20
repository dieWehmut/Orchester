import { describe, expect, it } from 'vitest'

import { createAppStores } from '../src/stores/app'

describe('app stores composition', () => {
  it('shares one HTTP client and in-memory CSRF token across domain stores', () => {
    const stores = createAppStores()

    expect(stores.http).toBe(stores.http)
    expect(stores.runs).toBeDefined()
    expect(stores.agents).toBeDefined()
    expect(stores.bootstrap.status.value).toBe('idle')
    expect(stores.sessions.status.value).toBe('idle')
    expect(stores.getCsrfToken()).toBeNull()
  })
})
