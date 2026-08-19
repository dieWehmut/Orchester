import { describe, expect, it } from 'vitest'

import { createAppRouter } from '../src/router'

describe('WebUI router', () => {
  it('redirects the root route to the workspace', async () => {
    const router = createAppRouter('memory')

    await router.push('/')
    await router.isReady()

    expect(router.currentRoute.value.name).toBe('workspace')
    expect(router.currentRoute.value.fullPath).toBe('/workspace')
  })

  it('resolves settings and unknown paths without eager view imports', () => {
    const router = createAppRouter('memory')

    expect(router.resolve('/settings').name).toBe('settings')
    expect(router.resolve('/missing').name).toBe('not-found')
  })
})
