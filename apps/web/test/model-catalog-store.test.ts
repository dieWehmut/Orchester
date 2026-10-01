import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { ModelsApi } from '../src/api/models'
import { useModelCatalogStore } from '../src/stores/model-catalog'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('model catalog Pinia store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  const provider = { provider: 'relay', name: 'Relay', base_url: 'https://example.com/v1', wire_api: 'responses' as const, model: 'test-model', api_key: 'fake-test-key' }

  it('adopts a saved provider only after the runtime confirms it, without retaining the key', async () => {
    const store = useModelCatalogStore()
    const saveProvider = vi.fn(async () => MODEL_CATALOG_FIXTURE)
    store.configure({ saveProvider } as unknown as ModelsApi)
    expect(await store.saveProvider(provider)).toBe(true)
    expect(store.catalog).toEqual(MODEL_CATALOG_FIXTURE)
    expect(store.status).toBe('ready')
    expect(JSON.stringify(store.$state)).not.toContain(provider.api_key)
  })

  it('preserves the previous catalog after a save failure', async () => {
    const store = useModelCatalogStore()
    store.configure({ catalog: async () => MODEL_CATALOG_FIXTURE, saveProvider: async () => { throw new TypeError('offline') } } as unknown as ModelsApi)
    await store.load()
    expect(await store.saveProvider(provider)).toBe(false)
    expect(store.catalog).toEqual(MODEL_CATALOG_FIXTURE)
    expect(store.status).toBe('stale')
  })

  it('does not restore a save response after the workspace has been reset', async () => {
    const store = useModelCatalogStore()
    let resolve!: (value: typeof MODEL_CATALOG_FIXTURE) => void
    store.configure({ saveProvider: () => new Promise((done) => { resolve = done }) } as unknown as ModelsApi)
    const saving = store.saveProvider(provider)
    store.reset()
    resolve(MODEL_CATALOG_FIXTURE)
    expect(await saving).toBe(false)
    expect(store.status).toBe('idle')
    expect(store.catalog).toBeNull()
  })

  it('loads the catalog and derives the active model context', async () => {
    const store = useModelCatalogStore()
    const api = {
      catalog: vi.fn(async () => MODEL_CATALOG_FIXTURE),
    } as unknown as ModelsApi

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
    } as unknown as ModelsApi

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
    } as unknown as ModelsApi

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

  it('takes the catalog the selection produced rather than the one clicked', async () => {
    const store = useModelCatalogStore()
    const selected = {
      ...MODEL_CATALOG_FIXTURE,
      active: {
        state: 'configured' as const,
        choice: { ...MODEL_CATALOG_FIXTURE.providers[0]!, model: 'gpt-review' },
      },
    }
    const api = {
      catalog: vi.fn(async () => MODEL_CATALOG_FIXTURE),
      select: vi.fn(async () => selected),
    } as unknown as ModelsApi

    store.configure(api)
    const ok = await store.select({ profile: 'review' })

    expect(ok).toBe(true)
    expect(api.select).toHaveBeenCalledWith({ profile: 'review' })
    expect(store.catalog).toEqual(selected)
    expect(store.status).toBe('ready')
  })

  it('keeps the model it had when the runtime refuses the choice', async () => {
    const store = useModelCatalogStore()
    const api = {
      catalog: vi.fn(async () => MODEL_CATALOG_FIXTURE),
      select: vi.fn(async () => {
        throw new TypeError('offline')
      }),
    } as unknown as ModelsApi

    store.configure(api)
    await store.load()
    const ok = await store.select({ provider: 'nowhere' })

    // A refused choice is not a state the picker may draw: the catalog stays as
    // the runtime last reported it, and the failure is what the reader is told.
    expect(ok).toBe(false)
    expect(store.catalog).toEqual(MODEL_CATALOG_FIXTURE)
    expect(store.status).toBe('stale')
    expect(store.error?.message).toBe('Unable to reach the Orchester runtime')
  })
})
