import { describe, expect, it } from 'vitest'

import { createAppPinia } from '../src/stores/pinia'

describe('application Pinia bootstrap', () => {
  it('creates an isolated container for each application instance', () => {
    const first = createAppPinia()
    const second = createAppPinia()

    expect(first).not.toBe(second)
    expect(first._s).not.toBe(second._s)
  })
})
