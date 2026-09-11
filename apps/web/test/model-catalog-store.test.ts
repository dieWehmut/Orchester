import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { ModelsApi } from '../src/api/models'
import { useModelCatalogStore } from '../src/stores/model-catalog'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('model catalog Pinia store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('loads the catalog and derives the active model context', async () => {
    const store = useModelCatalogStore()
    const api = {
      catalog: vi.fn(async () => MODEL_CATALOG_FIXTURE),
    } as ModelsApi

    store.configure(api)
    await store.load()

    expect(store.status).toBe('ready')
    expect(store.activeChoice?.model).toBe('gpt-5.6')
    expect(store.activeProvider?.id).toBe('openai')
    expect(store.catalog?.profiles.map((profile) => profile.profile)).toEqual(['review'])
    expect(store.error).toBeNull()
  })

  it('retains the last catalog and marks it stale after a refresh failure', async () => {
    const store = useModelCatalogStore()
    const api = {
      catalog: vi
        .fn()
        .mockResolvedValueOnce(MODEL_CATALOG_FIXTURE)
        .mockRejectedValueOnce(new TypeError('offline')),
    } as ModelsApi

    store.configure(api)
    await store.load()
    await store.load()

    expect(store.catalog).toEqual(MODEL_CATALOG_FIXTURE)
    expect(store.status).toBe('stale')
    expect(store.error?.message).toBe('Unable to reach the Orchester runtime')
  })

  it('reports an unavailable model API without discarding state transitions', async () => {
    const store = useModelCatalogStore()

    await store.load()

    expect(store.catalog).toBeNull()
    expect(store.status).toBe('error')
    expect(store.error?.message).toBe('Unable to reach the Orchester runtime')
  })

  it('resets catalog state while preserving the configured API', async () => {
    const store = useModelCatalogStore()
    const api = {
      catalog: vi.fn(async () => MODEL_CATALOG_FIXTURE),
    } as ModelsApi

    store.configure(api)
    await store.load()
    store.reset()

    expect(store.catalog).toBeNull()
    expect(store.status).toBe('idle')
    expect(store.error).toBeNull()

    await store.load()
    expect(api.catalog).toHaveBeenCalledTimes(2)
    expect(store.status).toBe('ready')
  })
})
