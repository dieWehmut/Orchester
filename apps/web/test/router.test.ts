import { describe, expect, it } from 'vitest'

import { createAppRouter } from '../src/router'

describe('WebUI router', () => {
  it('configures the root route to redirect to the workspace', () => {
    const router = createAppRouter('memory')
    const rootRoute = router.getRoutes().find((route) => route.path === '/')

    expect(rootRoute?.redirect).toEqual({ name: 'workspace' })
    expect(router.resolve('/workspace').name).toBe('workspace')
  })

  it('resolves settings and unknown paths without eager view imports', () => {
    const router = createAppRouter('memory')

    expect(router.resolve('/settings').name).toBe('settings')
    expect(router.resolve('/missing').name).toBe('not-found')
  })
})
