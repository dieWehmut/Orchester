import { describe, expect, it } from 'vitest'

import { Check, CircleAlert, X } from '@lucide/vue'

describe('icon dependency', () => {
  it('provides the shared lucide Vue icon components', () => {
    expect([Check, CircleAlert, X]).not.toContain(undefined)
  })
})
