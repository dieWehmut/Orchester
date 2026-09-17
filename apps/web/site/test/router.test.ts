import { createMemoryHistory } from 'vue-router'
import { describe, expect, it } from 'vitest'

import { createWebsiteRouter } from '../src/router'

describe('website router', () => {
  it.each([
    ['/', 'home'],
    ['/architecture', 'architecture'],
    ['/install', 'install'],
    ['/missing', 'not-found'],
  ])('resolves %s to the %s lazy route', (path, name) => {
    const router = createWebsiteRouter(createMemoryHistory())
    const route = router.resolve(path)
    const view = route.matched.at(-1)?.components?.default

    expect(route.name).toBe(name)
    expect(view).toBeTypeOf('function')
  })
})
